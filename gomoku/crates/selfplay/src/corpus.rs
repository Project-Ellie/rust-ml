//! The corpus: durable storage for many recorded games.
//!
//! A corpus is one directory holding fixed-capacity shards of
//! length-delimited bincode [`GameRecord`] values plus a JSON manifest. The
//! shard framing lives in [`train::storage`]; this module adds the corpus
//! discipline on top: one declared [`SeatPairing`] per corpus, an explicit
//! overwrite policy, and the manifest written last as the completion marker.
//!
//! Checked at read time are framing integrity, decodability, and — through
//! [`read_manifest`] only — the manifest's JSON shape. A corpus that reads
//! back cleanly from this API is *well-formed*, not *true*: move legality
//! and outcome correctness are the replay verifier's subject.

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::record::{GAME_RECORD_FORMAT_VERSION, GameRecord, SeatPairing};

/// The per-seat evaluator identities behind a corpus, recorded when any
/// seat ran something other than the shared baseline evaluator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeatEvaluators {
    /// Black's evaluator descriptor, e.g. `net:first-corpus@0.1.0`.
    pub black: String,
    /// White's evaluator descriptor.
    pub white: String,
}

/// Provenance summary of one corpus, serialized as `manifest.json`.
///
/// The shards are the data; the manifest is the provenance. It is a
/// completion marker, not a checksum, not a lock, and not a durability
/// guarantee. Field declaration order matters: the pretty JSON writer emits
/// fields in this order, and byte-exactness comparisons rely on it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    /// Durable game-record layout version, shared with every record header.
    pub format_version: u16,
    /// Seed of each game, in corpus order.
    pub seeds: Vec<u64>,
    /// The declared Black-vs-White seat configuration of the whole corpus.
    pub pairing: SeatPairing,
    /// Number of recorded games in the corpus.
    pub game_count: usize,
    /// Shard file names, in record order.
    pub shards: Vec<String>,
    /// Workspace package version at generation time.
    pub workspace_version: String,
    /// Per-seat evaluator identities; `None` for pure-baseline corpora.
    ///
    /// JSON-only, so skipping the absent field keeps baseline manifests
    /// byte-identical and lets older manifests parse.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluators: Option<SeatEvaluators>,
    /// Hard cap on post-ceremony plies; `None` for uncapped corpora.
    ///
    /// Capped games abort as draws once the limit is reached. The cap is
    /// recorded only in the manifest — never in the bincode record — so the
    /// shard layout stays compatible with older corpora. Skipping the absent
    /// field keeps pre-cap manifests parsing unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_plies: Option<usize>,
}

/// The explicit choice for handling a non-empty output directory.
///
/// The policy never crosses into a file; there is deliberately no `Default`
/// implementation — the safe default is embodied in [`write_corpus`], and
/// replacement is always an explicit, typed decision at the call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverwritePolicy {
    /// Refuse every non-empty output directory.
    Refuse,
    /// Remove the existing output directory before writing.
    Allow,
}

/// Options for [`write_corpus_with_options`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusWriteOptions {
    /// Maximum number of games in one shard; a maximum, not a padding
    /// requirement — the last shard may be short.
    pub shard_capacity: usize,
    /// How to handle a non-empty output directory.
    pub overwrite: OverwritePolicy,
    /// Per-seat evaluator identities recorded into the manifest; `None` for
    /// pure-baseline corpora.
    pub evaluators: Option<SeatEvaluators>,
    /// Hard cap on post-ceremony plies recorded into the manifest; `None`
    /// for uncapped corpora.
    pub max_plies: Option<usize>,
}

/// Preflight the overwrite gate of [`write_corpus_with_options`] without
/// writing anything.
///
/// Callers with an expensive generation phase can fail fast, before the
/// work starts; the write path keeps its own gate as the final word, so a
/// preflight never weakens the protection.
///
/// # Errors
/// Returns `AlreadyExists` when the directory is non-empty and the policy
/// is [`OverwritePolicy::Refuse`], or any error from inspecting the
/// directory. With [`OverwritePolicy::Allow`] the removal itself stays in
/// the write path; the preflight only checks inspectability.
pub fn check_writable(out_dir: &Path, overwrite: OverwritePolicy) -> io::Result<()> {
    match std::fs::read_dir(out_dir) {
        Ok(mut entries) => {
            if entries.next().transpose()?.is_some() && overwrite == OverwritePolicy::Refuse {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "refusing to write a corpus into a non-empty directory",
                ));
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    Ok(())
}

/// Write game records to fixed-capacity length-delimited bincode shards and
/// a JSON manifest, in the fixed phase order validate → gate → shards →
/// manifest. The order is the protection.
///
/// With [`OverwritePolicy::Allow`], the entire existing output directory is
/// removed before writing — a remove-then-write, not an atomic swap: a crash
/// between removal and the manifest loses the old corpus, and a failed
/// removal can leave the old directory partially removed, because directory
/// removal deletes entry by entry. Only validation failures before removal
/// guarantee the old corpus remains fully intact. Without that explicit
/// opt-in, any non-empty directory is refused before anything is touched.
///
/// The manifest is written last: a directory with shards but no manifest is
/// an interrupted write, and the manifest never claims shards that do not
/// exist. The returned [`Manifest`] is exactly what is on disk.
///
/// # Errors
///
/// Returns an error if validation fails (a zero shard capacity, an empty
/// record slice, or a record whose pairing differs from the first record's),
/// if the output directory cannot be inspected, created, or removed, if
/// manifest serialization fails, or if writing a shard or the manifest
/// fails.
pub fn write_corpus_with_options(
    records: &[GameRecord],
    out_dir: &Path,
    options: CorpusWriteOptions,
) -> io::Result<Manifest> {
    // Phase 1: validation, before anything on disk is touched.
    if options.shard_capacity == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "shard capacity must be greater than zero",
        ));
    }
    let Some(first) = records.first() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a corpus needs at least one game to declare its pairing",
        ));
    };
    let pairing = first.header.config;
    if records.iter().any(|record| record.header.config != pairing) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "every game in a corpus must use the declared pairing",
        ));
    }

    // Phase 2: the overwrite gate.
    check_writable(out_dir, options.overwrite)?;
    if matches!(options.overwrite, OverwritePolicy::Allow) && out_dir.try_exists()? {
        std::fs::remove_dir_all(out_dir)?;
    }

    // Phase 3: the shards.
    std::fs::create_dir_all(out_dir)?;
    let mut shards = Vec::new();
    for (index, chunk) in records.chunks(options.shard_capacity).enumerate() {
        let name = format!("shard-{index:03}.bin");
        train::storage::write_shard(chunk, &out_dir.join(&name))?;
        shards.push(name);
    }

    // Phase 4: the manifest, last.
    let manifest = Manifest {
        format_version: GAME_RECORD_FORMAT_VERSION,
        seeds: records.iter().map(|record| record.header.seed).collect(),
        pairing,
        game_count: records.len(),
        shards,
        workspace_version: env!("CARGO_PKG_VERSION").to_owned(),
        evaluators: options.evaluators.clone(),
        max_plies: options.max_plies,
    };
    let file = File::options()
        .write(true)
        .create_new(true)
        .open(out_dir.join("manifest.json"))?;
    serde_json::to_writer_pretty(file, &manifest)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    Ok(manifest)
}

/// Write game records with default overwrite protection.
///
/// Delegates to [`write_corpus_with_options`] with
/// [`OverwritePolicy::Refuse`]; replacement is always an explicit, typed
/// decision at the call site.
///
/// # Errors
///
/// See [`write_corpus_with_options`].
pub fn write_corpus(
    records: &[GameRecord],
    out_dir: &Path,
    shard_capacity: usize,
) -> io::Result<Manifest> {
    write_corpus_with_options(
        records,
        out_dir,
        CorpusWriteOptions {
            shard_capacity,
            overwrite: OverwritePolicy::Refuse,
            evaluators: None,
            max_plies: None,
        },
    )
}

/// Read the manifest of one corpus directory.
///
/// # Errors
///
/// Returns an error if `manifest.json` cannot be opened — a missing file is
/// the operating system's error, propagated unchanged — or if its JSON is
/// malformed (`InvalidData`).
pub fn read_manifest(corpus_dir: &Path) -> io::Result<Manifest> {
    let file = File::open(corpus_dir.join("manifest.json"))?;
    serde_json::from_reader(file).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

/// Read every game record of one corpus directory, in record order.
///
/// The directory listing keeps regular files whose names start with
/// `shard-` and end with `.bin` — so `manifest.json` and stray files are
/// ignored — sorts them lexicographically (the zero-padded names make this
/// the write order), and decodes each through
/// [`train::storage::read_shard`]. A directory without matching shards
/// yields an empty vector. Framing errors — torn prefixes, oversized
/// prefixes, undecodable payloads — propagate from the shared reader
/// unchanged.
///
/// This never opens the manifest: the read-back checks framing integrity
/// and decodability only. A corpus that reads back cleanly is *well-formed*,
/// not *true*.
///
/// # Errors
///
/// Returns an error if the directory cannot be listed — a missing directory
/// is the operating system's error, propagated unchanged — or if any shard
/// fails the shared reader's framing defenses.
pub fn read_corpus(corpus_dir: &Path) -> io::Result<Vec<GameRecord>> {
    let mut shards: Vec<PathBuf> = Vec::new();
    for entry in std::fs::read_dir(corpus_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let path = entry.path();
        if path.is_file() && name.starts_with("shard-") && name.ends_with(".bin") {
            shards.push(path);
        }
    }
    shards.sort();

    let mut records = Vec::new();
    for shard in shards {
        records.extend(train::storage::read_shard(&shard)?);
    }
    Ok(records)
}
