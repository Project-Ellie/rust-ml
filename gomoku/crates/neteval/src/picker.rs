//! Journal-aware checkpoint picker: list the `nettrain` run
//! directories under a root so model-selection UIs can offer them.
//!
//! A run directory qualifies when it holds both `model.mpk` and a
//! parseable `run.json`; anything else is skipped silently. Listing is
//! sorted by the journal file's modification time, most recent first.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use net::checkpoint::{self, RunJournal};

/// One loadable run directory with its journal.
#[derive(Debug, Clone)]
pub struct CheckpointInfo {
    /// The run directory (passed to [`crate::NetEvaluator::load`]).
    pub dir: PathBuf,
    /// The run journal: seed, configs, final metrics, stop reason.
    pub journal: RunJournal,
    /// Modification time of `run.json` (recency sort key).
    pub modified: SystemTime,
}

/// List the loadable run directories directly under `root`, most
/// recently modified first. A missing or unreadable `root` yields an
/// empty list — the picker is a convenience, not a gate.
#[must_use]
pub fn scan(root: &Path) -> Vec<CheckpointInfo> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };

    let mut infos = Vec::new();
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() || !dir.join("model.mpk").is_file() {
            continue;
        }
        let Ok(journal) = checkpoint::load_journal(&dir) else {
            continue;
        };
        let modified = std::fs::metadata(dir.join("run.json"))
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        infos.push(CheckpointInfo {
            dir,
            journal,
            modified,
        });
    }
    infos.sort_by_key(|i| std::cmp::Reverse(i.modified));
    infos
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{scratch_dir, write_tiny_run};

    #[test]
    fn scan_finds_valid_runs_and_skips_the_rest() {
        let root = scratch_dir("picker-scan");

        let valid_a = root.join("run-a");
        write_tiny_run(&valid_a);
        let valid_b = root.join("run-b");
        write_tiny_run(&valid_b);

        // Missing model.mpk: journal only.
        let journal_only = root.join("journal-only");
        std::fs::create_dir_all(&journal_only).unwrap();
        std::fs::copy(valid_a.join("run.json"), journal_only.join("run.json")).unwrap();

        // Corrupt journal.
        let corrupt = root.join("corrupt");
        std::fs::create_dir_all(&corrupt).unwrap();
        std::fs::write(corrupt.join("run.json"), b"not json").unwrap();
        std::fs::write(corrupt.join("model.mpk"), b"garbage").unwrap();

        // A plain file in the root.
        std::fs::write(root.join("README"), b"hello").unwrap();

        let infos = scan(&root);
        let names: Vec<_> = infos
            .iter()
            .map(|i| i.dir.file_name().unwrap().to_owned())
            .collect();
        assert_eq!(names.len(), 2, "got {names:?}");
        assert!(names.contains(&"run-a".into()));
        assert!(names.contains(&"run-b".into()));
        assert!(
            infos
                .iter()
                .all(|i| i.journal.model_config == net::network::ModelConfig::tiny())
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_sorts_most_recent_first() {
        let root = scratch_dir("picker-sort");
        let older = root.join("older");
        write_tiny_run(&older);
        // Ensure a strictly newer mtime on the second journal.
        std::thread::sleep(std::time::Duration::from_millis(20));
        let newer = root.join("newer");
        write_tiny_run(&newer);

        let infos = scan(&root);
        assert_eq!(infos.len(), 2);
        assert_eq!(infos[0].dir.file_name().unwrap(), "newer");
        assert_eq!(infos[1].dir.file_name().unwrap(), "older");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_missing_root_is_empty() {
        let root = scratch_dir("picker-missing");
        assert!(scan(&root).is_empty());
    }
}
