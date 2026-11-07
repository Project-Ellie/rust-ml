# Save/load, then optional AI resignation — solution

Status (2026-11-07): the owner's native save/load implementation landed on `main` at `06d9598`. The opt-in resignation feature, including CLI/history/version-2 persistence, is implemented in the working tree; both slices passed independent review. Fresh full-workspace release QA passed: 649 tests, zero failures, seven ignored, plus three doc tests; clippy, formatting, the engine-island check, and a genuine lean-CLI compile check passed. Builds used `gomoku/target/resignation-qa`, leaving the existing release executable and the owner's saved game untouched. The examples below are the original implementation proposal, not an exact transcription of current source. Existing learner code is the implementation reference; baseline fork bonuses and saved diagnostic results are outside this change.

## Part 1 — `save PATH` and `load PATH`

Save a standalone study: opening stones and side to move, every variation node in arena order, cursor, current seat configurations, defaults, simulation overrides, and the optional launch seed. Do not save evaluator objects, random-generator state, search information, puzzle packs, hints, or display state. Loading a puzzle study therefore needs no puzzle file and does not restore `hint`, `reveal`, `next`, or `prev`.

Paths are literal trimmed command remainders. `SaVe Games/My  Study.json` preserves `Games/My  Study.json`; quotes, `~`, environment variables, and shell escapes have no special meaning. Relative paths use the process working directory. Both commands refuse an unfinished Swap2 ceremony. A completed ceremony is an ordinary opening root.

A successful load replaces the session once, after complete validation, and resets shell/input state. A failed load changes neither session nor overlays. Save borrows the session immutably and preserves the view. Loaded `new` returns to the loaded root with the loaded seats/defaults, not the original application's launch state.

### 1. Engine prerequisite: imported roots must draw correctly

In [engine/src/board.rs](../../gomoku/crates/engine/src/board.rs), `Board::from_position` deliberately has an empty move log; replace only the draw condition in `Board::play`, retaining the preceding five-in-a-row check.

```rust
        } else if self.empty_moves().next().is_none() {
            self.status = Status::Draw;
        }
```

This replaces `else if self.moves.len() == 225`, not the win test. Occupancy, not post-opening move-log length, determines a full board. No engine resignation type or engine I/O is needed.

Add this regression inside the existing tests in [engine/src/board.rs](../../gomoku/crates/engine/src/board.rs), using the same repeating-pair pattern as the current guard draw fixture.

```rust
    #[test]
    fn imported_224_stone_root_draws_undoes_and_draws_again() {
        let last = Move::new(0, 0).unwrap();
        let mut black = Vec::new();
        let mut white = Vec::new();
        for row in 0..15 {
            for col in 0..15 {
                let mv = Move::new(row, col).unwrap();
                if mv == last {
                    continue;
                }
                if (row + 2 * col) % 4 < 2 {
                    black.push(mv);
                } else {
                    white.push(mv);
                }
            }
        }
        assert_eq!((black.len(), white.len()), (112, 112));
        let mut board = Board::from_position(&black, &white, Color::Black).unwrap();
        assert!(board.moves().is_empty());
        assert!(!board.has_five_any(Color::Black));
        assert!(!board.has_five_any(Color::White));
        assert_eq!(board.empty_moves().collect::<Vec<_>>(), vec![last]);
        let original = board.clone();
        for _ in 0..2 {
            board.play(last).unwrap();
            assert_eq!(board.status(), Status::Draw);
            assert_eq!(board.moves(), &[last]);
            assert_eq!(board.empty_moves().count(), 0);
            board.undo();
            assert_eq!(board, original);
            assert_eq!(board.status(), Status::Ongoing);
        }
    }
```

Do not substitute a checkerboard: its long diagonals already contain wins.

### 2. Command grammar and shell-only dispatch

In [cli/src/command.rs](../../gomoku/crates/cli/src/command.rs), import `std::path::PathBuf` and add these `Command` variants.

```rust
    /// Save the study through a filesystem-capable shell.
    Save {
        /// Literal destination path.
        path: PathBuf,
    },
    /// Load a study through a filesystem-capable shell.
    Load {
        /// Literal source path.
        path: PathBuf,
    },
```

In `parse`, insert this block immediately before `let lower = trimmed.to_ascii_lowercase();` in [cli/src/command.rs](../../gomoku/crates/cli/src/command.rs).

```rust
    let split = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
    let verb = &trimmed[..split];
    if verb.eq_ignore_ascii_case("save") || verb.eq_ignore_ascii_case("load") {
        let remainder = trimmed[split..].trim();
        if remainder.is_empty() {
            return Err(CommandError::Parse(format!("{verb} requires a path")));
        }
        let path = std::path::PathBuf::from(remainder);
        return Ok(if verb.eq_ignore_ascii_case("save") {
            Command::Save { path }
        } else {
            Command::Load { path }
        });
    }
```

The remainder is not tokenized; normal commands retain their existing parser. Update the parser's case-insensitivity documentation to exempt path arguments.

Add this error variant in [cli/src/command.rs](../../gomoku/crates/cli/src/command.rs).

```rust
    /// This command requires the shell's filesystem boundary.
    #[error("save/load must be executed through the shell")]
    ShellOnly,
```

Add this arm to `SessionCore::execute` in [cli/src/session.rs](../../gomoku/crates/cli/src/session.rs), leaving ordinary command dispatch unchanged.

```rust
            Command::Save { .. } | Command::Load { .. } => Err(CommandError::ShellOnly),
```

### 3. Version-1 history records

A data transfer object (DTO) is a plain owned serializable structure, not runtime state. Version 1 uses `format: "gomoku-study"`, `version: 1`, and denies unknown fields in its record structures. Reject future versions rather than silently dropping their meaning. Limit files to 4 MiB and trees to 10,000 nodes including the root.

The actual type is `history::History`. Its private `Node` arena has parent, move, and ordered children, with root ID zero and no selected-mainline field. Saving parent-before-child arena order and rebuilding in that order preserves node IDs and sibling creation order. Do not flatten the cursor path or invent a mainline selection field.

Register `pub(crate) mod record;` in [cli/src/history.rs](../../gomoku/crates/cli/src/history.rs), then create `gomoku/crates/cli/src/history/record.rs` with this complete implementation.

```rust
//! Checked, owned history records; no filesystem access.
use super::{History, PlayOutcome};
use engine::{Board, Color, Move};
use serde::{Deserialize, Serialize};

pub(crate) const MAX_NODES: usize = 10_000;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RootRecord {
    pub black: Vec<[u8; 2]>,
    pub white: Vec<[u8; 2]>,
    pub to_move: Color,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NodeRecord {
    pub parent: Option<u32>,
    pub square: Option<[u8; 2]>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HistoryRecord {
    pub root: RootRecord,
    pub nodes: Vec<NodeRecord>,
    pub cursor: u32,
}

fn checked_square([row, col]: [u8; 2]) -> Result<Move, String> {
    Move::new(row, col).ok_or_else(|| format!("square outside board: [{row},{col}]"))
}

fn checked_stones(
    squares: &[[u8; 2]],
    occupied: &mut [bool; 225],
) -> Result<Vec<Move>, String> {
    squares.iter().map(|&square| {
        let mv = checked_square(square)?;
        if std::mem::replace(&mut occupied[mv.index()], true) {
            return Err("duplicate or overlapping root cell".into());
        }
        Ok(mv)
    }).collect()
}

impl RootRecord {
    pub(crate) fn from_board(board: &Board) -> Self {
        let mut black = Vec::new();
        let mut white = Vec::new();
        for row in 0..15 {
            for col in 0..15 {
                let mv = Move::new(row, col).expect("bounded coordinates");
                match board.stone_at(mv) {
                    Some(Color::Black) => black.push([row, col]),
                    Some(Color::White) => white.push([row, col]),
                    None => {}
                }
            }
        }
        Self { black, white, to_move: board.to_move() }
    }

    pub(crate) fn to_board(&self) -> Result<Board, String> {
        if self.black.len() + self.white.len() > 225 {
            return Err("too many root stones".into());
        }
        let mut occupied = [false; 225];
        let black = checked_stones(&self.black, &mut occupied)?;
        let white = checked_stones(&self.white, &mut occupied)?;
        let board = Board::from_position(&black, &white, self.to_move)
            .map_err(|e| format!("invalid root: {e}"))?;
        if board.has_five_any(Color::Black) && board.has_five_any(Color::White) {
            return Err("both colors win at root".into());
        }
        Ok(board)
    }
}

impl HistoryRecord {
    pub(crate) fn capture(history: &History) -> Self {
        Self {
            root: RootRecord::from_board(&history.root_board),
            nodes: history.nodes.iter().map(|node| NodeRecord {
                parent: node.parent,
                square: node.mv.map(|mv| [mv.row(), mv.col()]),
            }).collect(),
            cursor: history.cursor,
        }
    }

    pub(crate) fn restore(&self) -> Result<History, String> {
        if self.nodes.is_empty() || self.nodes.len() > MAX_NODES {
            return Err("history must contain 1..=10000 nodes".into());
        }
        if self.cursor as usize >= self.nodes.len() {
            return Err("cursor outside history".into());
        }
        if self.nodes[0].parent.is_some() || self.nodes[0].square.is_some() {
            return Err("root node must have null parent and square".into());
        }
        let mut history = History::from_opening(&self.root.to_board()?);
        for (index, node) in self.nodes.iter().enumerate().skip(1) {
            let parent = node.parent.ok_or("non-root node needs parent")?;
            if parent as usize >= index {
                return Err(format!("node {index}: parent must precede child"));
            }
            let mv = checked_square(node.square.ok_or("non-root node needs square")?)?;
            // Child module access is intentional; never navigate untrusted IDs via var().
            history.cursor = parent;
            let outcome = history.play(mv)
                .map_err(|e| format!("node {index}: {e}"))?;
            if outcome != PlayOutcome::Moved(index as u32) {
                return Err(format!("node {index}: duplicate sibling move"));
            }
        }
        history.cursor = self.cursor;
        Ok(history)
    }
}
```

Coordinates pass `Move::new`; do not deserialize `Move` directly because its derived `u8` representation admits indices 225–255. Duplicate cells within either root list must also fail: `Board::from_position` otherwise collapses them into bits. Engine construction validates counts and side-to-move; the extra both-winners test removes an ambiguous root. This is position validation, not a retrograde proof of opening reachability.

Every branch is replayed, including branches hidden from the saved cursor. A child following a win/draw fails through `History::play`. A duplicate sibling fails because `FollowedExisting` is not `Moved(expected_id)`. The loader never uses `History::var`; its current direct bound check has an unrelated `index > children.len()` edge case. Keep that separate from this feature.

Opening roots are stored as stones, not `Board::moves()`. The reconstructed root has no move log and thus no invented last move. Descendant last moves, cursor depth, and board meaning survive; whole-`Board` equality against a root with a pre-existing log is not the round-trip contract.

### 4. Pure session export/import

Register `pub mod record;` with `/// Native study record conversion, without filesystem access.` in [cli/src/session.rs](../../gomoku/crates/cli/src/session.rs), then create `gomoku/crates/cli/src/session/record.rs`.

```rust
//! Native version-1 study records; pure conversion and validation.
use super::{SessionCore, SessionInit};
use crate::actor::{AiConfig, EvaluatorKind, Occupant};
use crate::history::record::HistoryRecord;
use serde::{Deserialize, Serialize};

/// Maximum encoded study size, including whitespace.
pub const MAX_RECORD_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StudyRecord {
    format: String,
    version: u32,
    history: HistoryRecord,
    black: Occupant,
    white: Occupant,
    default_config: AiConfig,
    black_sims: Option<u32>,
    white_sims: Option<u32>,
    seed: Option<u64>,
}

fn validate_config(config: &AiConfig) -> Result<(), String> {
    if config.simulations == 0 {
        return Err("simulations must be positive".into());
    }
    if !config.c_puct.is_finite() || config.c_puct < 0.0 {
        return Err("c_puct must be finite and nonnegative".into());
    }
    if !config.temperature.is_finite() || config.temperature < 0.0 {
        return Err("temperature must be finite and nonnegative".into());
    }
    if !config.noise_epsilon.is_finite() || !(0.0..=1.0).contains(&config.noise_epsilon) {
        return Err("noise_epsilon must be finite and in [0,1]".into());
    }
    if config.evaluator == EvaluatorKind::Baseline && !cfg!(feature = "baseline") {
        return Err("record requires the baseline feature".into());
    }
    Ok(())
}

fn validate_occupant(occupant: &Occupant) -> Result<(), String> {
    if let Occupant::Ai(config) = occupant {
        validate_config(config)?;
    }
    Ok(())
}

impl StudyRecord {
    fn validate_configs(&self) -> Result<(), String> {
        validate_occupant(&self.black)?;
        validate_occupant(&self.white)?;
        validate_config(&self.default_config)?;
        if self.black_sims == Some(0) || self.white_sims == Some(0) {
            return Err("simulation overrides must be positive".into());
        }
        Ok(())
    }
}

impl SessionCore {
    /// Export a bounded standalone study without changing runtime state.
    ///
    /// # Errors
    /// Rejects pending Swap2, invalid configurations, oversized trees or JSON.
    pub fn export_record_json(&self) -> Result<String, String> {
        if self.opening.is_some() {
            return Err("save/load unavailable during the Swap2 ceremony".into());
        }
        let record = StudyRecord {
            format: "gomoku-study".into(),
            version: 1,
            history: HistoryRecord::capture(&self.history),
            black: self.black.clone(),
            white: self.white.clone(),
            default_config: self.default_config.clone(),
            black_sims: self.black_sims,
            white_sims: self.white_sims,
            seed: self.init.seed,
        };
        record.validate_configs()?;
        if record.history.nodes.len() > crate::history::record::MAX_NODES {
            return Err("too many history nodes".into());
        }
        let text = serde_json::to_string_pretty(&record).map_err(|e| e.to_string())?;
        if text.len() > MAX_RECORD_BYTES {
            return Err("study exceeds 4 MiB".into());
        }
        Ok(text)
    }

    /// Build a replacement session after validating the complete record.
    ///
    /// # Errors
    /// Rejects unsupported formats, malformed JSON, invalid trees or configs.
    pub fn import_record_json(text: &str) -> Result<Self, String> {
        if text.len() > MAX_RECORD_BYTES {
            return Err("study exceeds 4 MiB".into());
        }
        let record: StudyRecord = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if record.format != "gomoku-study" || record.version != 1 {
            return Err("unsupported study format/version".into());
        }
        record.validate_configs()?;
        let history = record.history.restore()?;
        // All untrusted input is checked before any evaluator construction.
        let opening = history.root_board().clone();
        let init = SessionInit {
            black: record.black,
            white: record.white,
            default_config: record.default_config,
            black_sims: record.black_sims,
            white_sims: record.white_sims,
            seed: record.seed,
            opening,
            swap2: false,
            puzzle: None,
        };
        let mut candidate = Self::new(init);
        candidate.history = history;
        Ok(candidate)
    }
}
```

Because `session::record` cannot access `History`'s private root, add this crate-private accessor inside `impl History` in [cli/src/history.rs](../../gomoku/crates/cli/src/history.rs).

```rust
    pub(crate) fn root_board(&self) -> &Board {
        &self.root_board
    }
```

The history child module itself can access its owner's private fields. `SessionCore::new` constructs one player per AI seat; do not create a provisional session and then call `set_occupant` again.

The existing `Occupant` and `AiConfig` already derive serde; `EvaluatorKind` serializes as `"Tactics"` or `"Baseline"`, `Occupant::Human` as `"Human"`, and an AI as an externally tagged `Ai` object containing its configuration. `seed` is an optional original launch seed, not a random-generator checkpoint. Loading reseeds from `Some(seed)`; `None` obtains fresh randomness. A generated effective seed is not retained by today's session and cannot be recovered here. Branch navigation and `new` keep their current RNG behavior; this solution adds no RNG rewind.

This is a complete version-1 example with two sibling children, cursor on the second, in the existing serde wire format.

```json
{
  "format": "gomoku-study",
  "version": 1,
  "history": {
    "root": {"black": [], "white": [], "to_move": "Black"},
    "nodes": [
      {"parent": null, "square": null},
      {"parent": 0, "square": [7, 7]},
      {"parent": 0, "square": [7, 8]}
    ],
    "cursor": 2
  },
  "black": "Human",
  "white": {
    "Ai": {
      "evaluator": "Tactics", "simulations": 400, "c_puct": 1.5,
      "temperature": 0.0, "noise_epsilon": 0.0, "explicit_selection": false
    }
  },
  "default_config": {
    "evaluator": "Tactics", "simulations": 400, "c_puct": 1.5,
    "temperature": 0.0, "noise_epsilon": 0.0, "explicit_selection": false
  },
  "black_sims": null,
  "white_sims": 400,
  "seed": 3477
}
```

### 5. Bounded filesystem boundary

Create `gomoku/crates/cli/src/game_file.rs` with this implementation, using only existing dependencies and `std`.

```rust
//! Bounded study-file reads and same-directory replacement writes.
use crate::session::record::MAX_RECORD_BYTES;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

/// Read at most the native record limit plus one sentinel byte.
///
/// # Errors
/// Returns filesystem, size, or UTF-8 errors as displayable text.
pub fn read(path: &Path) -> Result<String, String> {
    let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut bytes = Vec::new();
    file.take((MAX_RECORD_BYTES + 1) as u64).read_to_end(&mut bytes)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if bytes.len() > MAX_RECORD_BYTES {
        return Err("study exceeds 4 MiB".into());
    }
    String::from_utf8(bytes).map_err(|e| format!("study is not UTF-8: {e}"))
}

fn create_temp(path: &Path) -> Result<(PathBuf, File), String> {
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = path.file_name().ok_or("destination needs a filename")?;
    for _ in 0..128 {
        let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let mut temp_name = name.to_os_string();
        temp_name.push(format!(".tmp-{}-{serial}", std::process::id()));
        let temp = parent.join(temp_name);
        match OpenOptions::new().write(true).create_new(true).open(&temp) {
            Ok(file) => return Ok((temp, file)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("{}: {e}", temp.display())),
        }
    }
    Err("could not create a unique study temporary file".into())
}

/// Replace a study file only after writing, flushing, syncing and closing.
///
/// # Errors
/// Returns size or filesystem errors; removes an unsuccessful temporary file.
pub fn write(path: &Path, text: &str) -> Result<(), String> {
    if text.len() > MAX_RECORD_BYTES {
        return Err("study exceeds 4 MiB".into());
    }
    let (temp, mut file) = create_temp(path)?;
    let written = file.write_all(text.as_bytes())
        .and_then(|()| file.flush())
        .and_then(|()| file.sync_all());
    drop(file);
    let result = written.and_then(|()| fs::rename(&temp, path));
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result.map_err(|e| format!("{}: {e}", path.display()))
}
```

Serialization and the size check happen before opening a destination or temporary file. `create_new` prevents clobbering a colliding temp name; PID/counter naming consumes no session randomness. Same-directory rename gives normal macOS/Unix atomic replacement, not a power-loss durability guarantee: the directory is not synced. Do not claim identical replacement behavior on every platform, lock concurrent writers, or create parent directories silently.

Create `gomoku/crates/cli/src/shell_command.rs` with this shared dispatch implementation.

```rust
//! Filesystem-capable command dispatch shared by every shell.
use crate::command::{Command, Event};
use crate::game_file;
use crate::session::SessionCore;

/// Events and shell feedback from one successful command.
#[derive(Debug, Default)]
pub struct SharedShellReply {
    /// Ordinary core events; save/load do not fabricate game events.
    pub events: Vec<Event>,
    /// Optional filesystem-operation feedback.
    pub message: Option<String>,
    /// True only after a successful replacement load.
    pub loaded: bool,
}

/// Execute shell file commands, forwarding all others to the pure core.
///
/// # Errors
/// Reports command, validation, or filesystem errors without replacing the core.
pub fn execute(core: &mut SessionCore, command: Command) -> Result<SharedShellReply, String> {
    match command {
        Command::Save { path } => {
            let text = core.export_record_json()?;
            game_file::write(&path, &text)?;
            Ok(SharedShellReply {
                message: Some(format!("saved {}", path.display())),
                ..SharedShellReply::default()
            })
        }
        Command::Load { path } => {
            if core.snapshot().opening.is_some() {
                return Err("save/load unavailable during the Swap2 ceremony".into());
            }
            let text = game_file::read(&path)?;
            let candidate = SessionCore::import_record_json(&text)?;
            *core = candidate;
            Ok(SharedShellReply {
                message: Some(format!("loaded {}", path.display())),
                loaded: true,
                ..SharedShellReply::default()
            })
        }
        other => core.execute(other).map(|events| SharedShellReply {
            events,
            ..SharedShellReply::default()
        }).map_err(|e| e.to_string()),
    }
}
```

Register the two public modules in [cli/src/lib.rs](../../gomoku/crates/cli/src/lib.rs).

```rust
/// Bounded native study-file I/O.
pub mod game_file;
/// Filesystem-capable shell command dispatch.
pub mod shell_command;
```

### 6. All three front-end paths

In [cli/src/tui/study.rs](../../gomoku/crates/cli/src/tui/study.rs), replace the complete `InputOutcome::Command(cmd)` arm with this block.

```rust
            InputOutcome::Command(cmd) => match crate::shell_command::execute(core, cmd) {
                Ok(reply) => {
                    if reply.loaded {
                        shell = ShellState::default();
                        state = InputState::new();
                        events.clear();
                        message = None;
                    }
                    record_events(&mut events, &mut message, &reply.events);
                    if let Some(text) = reply.message {
                        message = Some(text);
                    }
                }
                Err(err) => message = Some(err),
            },
```

Help/quit interception remains above this arm. `loaded` is set only after assignment; failure may update the error message, not the board cursor, value-strip toggle, policy overlay, proof overlay, or event history. A successful load clears old puzzle/TSS/policy display material as well as recent messages. Save does not clear any of them.

In [cli/src/repl.rs](../../gomoku/crates/cli/src/repl.rs), replace `Ok(cmd) => match core.execute(cmd)` and its entire match body with this block.

```rust
        Ok(cmd) => match crate::shell_command::execute(core, cmd) {
            Ok(reply) => {
                if reply.events.is_empty() && reply.message.is_none() {
                    outcome.lines.push("(no events)".to_string());
                }
                for event in &reply.events {
                    outcome.lines.push(format!("  {event}"));
                }
                if let Some(message) = reply.message {
                    outcome.lines.push(message);
                }
                outcome.lines.extend(print_state(core, ""));
            }
            Err(e) => outcome.lines.push(format!("Error {e}")),
        },
```

The separate demo still directly executes commands; replace its corresponding `Ok(cmd)` arm in [cli/src/bin/sessiondemo.rs](../../gomoku/crates/cli/src/bin/sessiondemo.rs) with this block.

```rust
        Ok(cmd) => match cli::shell_command::execute(core, cmd) {
            Ok(reply) => {
                if reply.events.is_empty() && reply.message.is_none() {
                    println!("(no events)");
                }
                for event in &reply.events {
                    println!("  {event}");
                }
                if let Some(message) = reply.message {
                    println!("{message}");
                }
                print_state(core, "");
            }
            Err(e) => println!("Error {e}"),
        },
```

Add `save PATH` / `load PATH` to `repl::HELP`, which the study help already reuses, and to the demo's separate `HELP`; describe literal paths and pending-Swap2 refusal once in each. No additional UI command parser is needed: raw `:` input and line input already call `command::parse`.

### 7. Focused save/load tests

Append this test module to `gomoku/crates/cli/src/history/record.rs`.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branches_ids_sibling_order_and_cursor_round_trip() {
        let mut history = History::new();
        history.play(Move::new(7, 7).unwrap()).unwrap();
        history.play(Move::new(7, 8).unwrap()).unwrap();
        history.back(1);
        history.play(Move::new(8, 8).unwrap()).unwrap();
        history.back(1);
        let record = HistoryRecord::capture(&history);
        let restored = record.restore().unwrap();
        assert_eq!(restored, history);
        assert_eq!(restored.cursor(), 1);
        assert_eq!(restored.children(1), vec![
            Move::new(7, 8).unwrap(), Move::new(8, 8).unwrap(),
        ]);
        let mut bad_cursor = record.clone();
        bad_cursor.cursor = bad_cursor.nodes.len() as u32;
        assert!(bad_cursor.restore().is_err());
        let mut duplicate = record;
        duplicate.nodes[3].square = duplicate.nodes[2].square;
        assert!(duplicate.restore().is_err());
    }

    #[test]
    fn illegal_hidden_branch_is_checked_even_when_cursor_is_root() {
        let mut history = History::new();
        history.play(Move::new(7, 7).unwrap()).unwrap();
        history.play(Move::new(7, 8).unwrap()).unwrap();
        history.back(2);
        let mut record = HistoryRecord::capture(&history);
        record.nodes[2].square = Some([7, 7]);
        assert_eq!(record.cursor, 0);
        assert!(record.restore().is_err());
        record.nodes[2].square = Some([15, 0]);
        assert!(record.restore().is_err());
        record.nodes[2].square = Some([0, 255]);
        assert!(record.restore().is_err());
    }
}
```

Add this test in the parser's existing tests in [cli/src/command.rs](../../gomoku/crates/cli/src/command.rs).

```rust
    #[test]
    fn file_paths_preserve_case_and_internal_spaces() {
        assert_eq!(parse("  SaVe   Games/My  Study.JSON \t").unwrap(),
            Command::Save { path: "Games/My  Study.JSON".into() });
        assert_eq!(parse("LoAd\tMixed Case/a.json").unwrap(),
            Command::Load { path: "Mixed Case/a.json".into() });
        assert!(parse("save \t").is_err());
        assert!(parse("load").is_err());
        assert_eq!(parse("save ~/x.json").unwrap(),
            Command::Save { path: "~/x.json".into() });
    }
```

Append these tests to `gomoku/crates/cli/src/session/record.rs`, where ancestor-private RNG and init fields are accessible.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Command;
    use engine::Move;
    use rand::{RngExt, SeedableRng, rngs::StdRng};

    #[test]
    fn save_does_not_advance_rng_and_load_reseeds_instead_of_continuing() {
        let init = SessionInit { seed: Some(3477), ..SessionInit::hotseat() };
        let mut core = SessionCore::new(init);
        let _ = core.rng.random::<u64>();
        let mut expected_save_rng = StdRng::seed_from_u64(3477);
        let _ = expected_save_rng.random::<u64>();
        let text = core.export_record_json().unwrap();
        assert_eq!(core.rng.random::<u64>(), expected_save_rng.random::<u64>());
        let mut loaded = SessionCore::import_record_json(&text).unwrap();
        let mut fresh = StdRng::seed_from_u64(3477);
        assert_eq!(loaded.rng.random::<u64>(), fresh.random::<u64>());
        assert_eq!(loaded.init.seed, Some(3477));
        let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
        value["seed"] = serde_json::Value::Null;
        let unseeded = SessionCore::import_record_json(&value.to_string()).unwrap();
        assert_eq!(unseeded.init.seed, None);
        // No probabilistic assertion about two fresh generators differing.
    }

    #[test]
    fn loaded_new_uses_loaded_root_and_current_seats() {
        let mut opening = engine::Board::new();
        opening.play(Move::new(7, 7).unwrap()).unwrap();
        let mut source = SessionCore::new(SessionInit::hotseat_from_opening(&opening));
        source.execute(Command::Play { mv: Move::new(7, 8).unwrap() }).unwrap();
        let text = source.export_record_json().unwrap();
        let mut loaded = SessionCore::import_record_json(&text).unwrap();
        loaded.execute(Command::New).unwrap();
        assert_eq!(loaded.snapshot().ply, 0);
        assert_eq!(loaded.board().stone_at(Move::new(7, 7).unwrap()), Some(engine::Color::Black));
        assert_eq!(loaded.board().stone_at(Move::new(7, 8).unwrap()), None);
        assert!(!loaded.init.swap2);
        assert!(loaded.puzzle().is_none());
    }
}
```

Append these tests to `gomoku/crates/cli/src/shell_command.rs`; they create only isolated temporary test files when the proposed tests are later run.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SessionInit;
    use engine::Move;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn scratch() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("gomoku-record-test-{}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap();
        path
    }

    #[test]
    fn failed_load_rolls_back_and_pending_swap2_refuses_both_commands() {
        let dir = scratch();
        let path = dir.join("Broken Study.json");
        std::fs::write(&path, b"{broken").unwrap();
        let mut core = SessionCore::new(SessionInit::hotseat());
        core.execute(Command::Play { mv: Move::new(7, 7).unwrap() }).unwrap();
        let before = core.export_record_json().unwrap();
        assert!(execute(&mut core, Command::Load { path: path.clone() }).is_err());
        assert_eq!(core.export_record_json().unwrap(), before);
        let mut pending = SessionCore::new(SessionInit::hotseat().with_swap2(true));
        let snapshot = pending.snapshot();
        for cmd in [Command::Save { path: path.clone() }, Command::Load { path: path.clone() }] {
            assert!(execute(&mut pending, cmd).is_err());
            assert_eq!(pending.snapshot(), snapshot);
        }
        assert_eq!(std::fs::read(&path).unwrap(), b"{broken");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
```

Remaining repetitive cases belong in the existing history/session/study test harnesses: empty/terminal root, duplicate roots, overlapping colors, bad counts, both winners, missing parent/square, forward parent/cycle, post-terminal hidden child, empty/10,001-node tree, oversized/invalid-UTF-8 input, future version, invalid numeric configs and unavailable default Baseline, overwrite/read failures, temp cleanup, exact 4-MiB boundary, success reply `loaded`, and save preserving all shell toggles. Script `study_loop` with its recording screen: activate policy/proof/value state, fail a load and check it survives, then load successfully and check those states and old recent messages disappear. Test both `repl::process_line` and the demo path, not just core conversion. Do not use a live game as a test fixture.

## Part 2 — opt-in heuristic AI resignation

`--resign` defaults to false and applies to either evaluator. After a real search, before root noise or sampling, rank all root edges by visits descending, prior descending, then move index ascending. Check the top `min(5, legal_count)` moves. Resign only when every checked successor has a verified opponent continuous-four (VCF) certificate. A VCF certificate is the existing engine threat-space-search proof accepted by `verify_line`; no new prover or search family is introduced.

Each successor gets the existing baseline guard budget: 1,000 nodes / 10 plies. These are at most five **additional** opponent probes, not a wall-clock bound; verification is extra work. Unknown, exhausted, malformed, or unverifiable results all cancel resignation. An own immediate win anywhere on the board cancels it, even outside the top five. A certified own proof still bypasses search exactly as before.

This is deliberately a **heuristic concession**: a lower-ranked unchecked move might save the game. It is not an all-legal-moves loss proof and uses no value threshold. The existing guard and its fallback remain unchanged; do not derive resignation merely from `GuardOutcome::AllRefuted` or `BudgetExhausted`.

### 8. Explicit decision action

In [player/src/lib.rs](../../gomoku/crates/player/src/lib.rs), add `mod resign;`, add the action/reason types below, and replace `Decision` with the shown definition.

```rust
/// Why the player voluntarily conceded, not an engine adjudication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResignReason {
    /// Every checked top-ranked candidate has a verified opponent VCF.
    VerifiedOpponentVcf {
        /// Checked candidates; weaker unchecked moves may still defend.
        candidates: usize,
    },
}

/// A decision either plays a stone or concedes without playing one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionAction {
    /// Play this legal move.
    Play(engine::Move),
    /// Concede under an explicitly enabled heuristic.
    Resign(ResignReason),
}

/// One player action and statistics from the real search, when it ran.
#[derive(Debug, Clone, PartialEq)]
pub struct Decision {
    /// Play or concede; no sentinel move represents resignation.
    pub action: DecisionAction,
    /// Real search stats; absent only for the certified own-proof bypass.
    pub search: Option<SearchStats>,
    /// Selection diagnostics; absent when no proposal was sampled.
    pub guard: Option<GuardDiagnostics>,
}
```

Add this field to `PlayerOptions`, with `resign: false` in its `Default` implementation in [player/src/lib.rs](../../gomoku/crates/player/src/lib.rs).

```rust
    /// Enable heuristic resignation after verified top-candidate refutations.
    pub resign: bool,
```

Create `gomoku/crates/player/src/resign.rs` with this complete private helper.

```rust
//! Conservative admission for an explicitly enabled heuristic concession.
use crate::{GuardConfig, ResignReason};
use engine::{Board, Color, Proof, SearchBudget, Status};
use mcts::Edge;

pub(crate) fn consider(
    board: &Board,
    edges: &[Edge],
    mut probe: impl FnMut(&Board, Color, SearchBudget) -> Option<Proof>,
) -> Option<ResignReason> {
    if board.status() != Status::Ongoing {
        return None;
    }
    if !engine::immediate_wins(board, board.to_move()).is_empty() {
        return None;
    }
    // On an ongoing board every empty cell is legal; no temporary vector.
    let legal_count = board.empty_moves().count();
    if legal_count == 0 || edges.len() != legal_count {
        return None;
    }
    let mut seen = [false; 225];
    for edge in edges {
        if !board.is_legal(edge.mv) || !edge.prior.is_finite()
            || std::mem::replace(&mut seen[edge.mv.index()], true)
        {
            return None;
        }
    }
    // Legal unique edges with equal cardinality cover every legal move.
    let mut ranked: Vec<_> = edges.iter().collect();
    ranked.sort_by(|a, b| b.n.cmp(&a.n)
        .then_with(|| b.prior.partial_cmp(&a.prior).expect("finite priors"))
        .then_with(|| a.mv.index().cmp(&b.mv.index())));
    let candidates = ranked.len().min(5);
    for edge in ranked.into_iter().take(candidates) {
        let mut after = board.clone();
        if after.play(edge.mv).is_err() || after.status() != Status::Ongoing {
            return None;
        }
        let opponent = after.to_move();
        let proof = probe(&after, opponent, GuardConfig::baseline().budget)?;
        if proof.winner != opponent {
            return None;
        }
        let first = *proof.line.first()?;
        if !after.is_legal(first) || !engine::verify_line(&after, &proof) {
            return None;
        }
    }
    Some(ResignReason::VerifiedOpponentVcf { candidates })
}
```

No RNG argument belongs in this helper. Requiring all successor statuses to remain ongoing is conservative: an own terminal win or draw always cancels the concession, and an unexpected terminal opponent win also cancels it.

Replace the own-proof `Decision` constructor in [player/src/lib.rs](../../gomoku/crates/player/src/lib.rs) with this block.

```rust
                return Ok(Decision {
                    action: DecisionAction::Play(mv),
                    search: None,
                    guard: Some(GuardDiagnostics::new(mv, GuardOutcome::ProvenWin)),
                });
```

Immediately after `mcts::search` returns `tree, root`, insert this block in [player/src/lib.rs](../../gomoku/crates/player/src/lib.rs).

```rust
        if options.resign {
            if let Some(reason) = resign::consider(
                board, tree.node(root).edges(), engine::prove_forced_win,
            ) {
                return Ok(Decision {
                    action: DecisionAction::Resign(reason),
                    search: Some(SearchStats::from_searched(&tree, root, board)),
                    guard: None,
                });
            }
        }
```

Keep the subsequent noise → sampling → existing `guard::choose` block unchanged, replacing only its final constructor in [player/src/lib.rs](../../gomoku/crates/player/src/lib.rs).

```rust
        Ok(Decision {
            action: DecisionAction::Play(mv),
            search: Some(SearchStats::from_searched(&tree, root, board)),
            guard: Some(guard),
        })
```

Thus failed resignation admission draws no random numbers and falls through to precisely the previous selection pipeline. On resignation the real statistics are pre-noise root statistics; no proposal exists, so guard diagnostics must be `None`, not a fictitious proposal or fabricated disabled guard.

### 9. CLI configuration plumbing

In [cli/src/actor.rs](../../gomoku/crates/cli/src/actor.rs), add the field below to `AiConfig` and `resign: false` to its `Default` implementation.

```rust
    /// Opt in to heuristic concession after verified opponent VCF refutations.
    #[serde(default)]
    pub resign: bool,
```

In [cli/src/main.rs](../../gomoku/crates/cli/src/main.rs), add this field to `SearchArgs`, add `resign: search.resign` to `ai_config_from`, and add `resign: false` to `bare_search_args`.

```rust
    /// Allow heuristic resignation if every top-five candidate is VCF-refuted.
    #[arg(long)]
    resign: bool,
```

Add `resign: effective_config.resign` to the explicit `PlayerOptions` literal in `SessionCore::execute_step` in [cli/src/session.rs](../../gomoku/crates/cli/src/session.rs). Do not tie it to `EvaluatorKind::Baseline`: tactics seats also honor the opt-in. Existing own-TSS and guard presets still depend on evaluator kind exactly as before. `resign` does not set `explicit_selection` or alter the opening schedule.

### 10. History-owned concession annotations

A concession belongs to the **current position node**, not a fake move or child. Back/forward/var remain legal; returning to an annotated node restores the concession. Existing children are retained and can be navigated, although play/step at the annotated node refuse. Reset/new creates fresh nodes and clears annotations.

Add this type near `NodeId`, add `resignation: Option<Resignation>` to private `Node`, and initialize it to `None` in both root and appended-node constructors in [cli/src/history.rs](../../gomoku/crates/cli/src/history.rs).

```rust
/// A historical voluntary concession attached to a position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resignation {
    /// Side that conceded, which remains the board's side to move.
    pub by: engine::Color,
    /// Number of top-ranked candidates refuted by verified opponent VCFs.
    pub candidates: usize,
}
```

Add these accessors inside `impl History` in [cli/src/history.rs](../../gomoku/crates/cli/src/history.rs).

```rust
    /// Concession, if any, at the current position node.
    #[must_use]
    pub fn resignation(&self) -> Option<Resignation> {
        self.nodes[self.cursor as usize].resignation
    }

    pub(crate) fn set_resignation(&mut self, resignation: Resignation) {
        self.nodes[self.cursor as usize].resignation = Some(resignation);
    }
```

Add the following method to `SessionCore` in [cli/src/session.rs](../../gomoku/crates/cli/src/session.rs).

```rust
    /// Whether play/step are finished here, including a voluntary concession.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.history.board().status() != Status::Ongoing || self.history.resignation().is_some()
    }
```

In both `execute_step` and `execute_play`, replace their pre-move `if board.status() != Status::Ongoing` with `if self.is_finished()`. Keep Swap2 routing first. Do not replace the **post-move** board-status checks that emit real engine `GameOver` events.

Add this method inside `impl SessionCore`, then call it from `execute_step` as shown next in [cli/src/session.rs](../../gomoku/crates/cli/src/session.rs).

```rust
    fn record_resignation(
        &mut self,
        reason: player::ResignReason,
        stats: player::SearchStats,
        config: &AiConfig,
    ) -> Vec<Event> {
        let player::ResignReason::VerifiedOpponentVcf { candidates } = reason;
        let resignation = crate::history::Resignation {
            by: self.history.board().to_move(), candidates,
        };
        self.history.set_resignation(resignation);
        self.search_info.insert(self.history.cursor(), SearchInfo::from_stats(stats, config));
        vec![Event::Resigned { by: resignation.by, candidates }]
    }
```

Replace `let mv = decision.mv;` in `execute_step` in [cli/src/session.rs](../../gomoku/crates/cli/src/session.rs) with this block; keep the remaining ordinary-move handling, including storing search info on the resulting child.

```rust
        let mv = match decision.action {
            player::DecisionAction::Play(mv) => mv,
            player::DecisionAction::Resign(reason) => {
                let stats = decision.search.expect("resignation follows a real search");
                return Ok(self.record_resignation(reason, stats, &effective_config));
            }
        };
```

The `Resign` branch returns only `Event::Resigned`: no stone, no cursor movement, no `Moved`, and no `GameOver`. The stored statistics describe the unchanged current board from the original search side's perspective. Normal searches keep their existing child-node storage; own-proof moves keep `search: None` and call `execute_play` as before.

### 11. Snapshot, events, and effective-finish rendering

Add `pub resignation: Option<crate::history::Resignation>` with a doc comment to `Snapshot`, set it from `self.history.resignation()` in `SessionCore::snapshot`, and update both manual serde implementations in [cli/src/session.rs](../../gomoku/crates/cli/src/session.rs).

```rust
        let field_count = 6 + usize::from(self.opening.is_some())
            + usize::from(self.resignation.is_some());
```

After serializing optional `opening`, serialize optional resignation in [cli/src/session.rs](../../gomoku/crates/cli/src/session.rs).

```rust
        if let Some(resignation) = &self.resignation {
            state.serialize_field("resignation", resignation)?;
        }
```

Add `resignation: Option<crate::history::Resignation>` to the local `SnapshotHelper` and `resignation: helper.resignation` to its `Snapshot` constructor in [cli/src/session.rs](../../gomoku/crates/cli/src/session.rs); an absent option reads as `None`.

Add this event variant and its display arm in [cli/src/command.rs](../../gomoku/crates/cli/src/command.rs).

```rust
    /// The AI voluntarily conceded; the underlying board remains ongoing.
    Resigned {
        /// Side conceding.
        by: Color,
        /// Top-ranked candidates with verified opponent VCF refutations.
        candidates: usize,
    },
```

```rust
            Event::Resigned { by, candidates } => write!(f,
                "{by:?} resigned: {candidates} top candidates VCF-refuted (heuristic; other moves unchecked)"),
```

In `position_lines` in [cli/src/tui/render.rs](../../gomoku/crates/cli/src/tui/render.rs), replace the `match snapshot.status` block with this effective-outcome display.

```rust
    if let Some(resignation) = snapshot.resignation {
        lines.push(format!("{:?} resigned ({} candidates)", resignation.by, resignation.candidates));
        lines.push("Heuristic; other moves unchecked".to_string());
    } else {
        match snapshot.status {
            Status::Ongoing => lines.push(format!("To move: {:?}", board.to_move())),
            Status::Won(color) => lines.push(format!("Game over: {color:?} wins")),
            Status::Draw => lines.push("Game over: draw".to_string()),
        }
    }
```

Add the same concession line after the ordinary status line in `repl::print_state` and the demo's `print_state` in [cli/src/repl.rs](../../gomoku/crates/cli/src/repl.rs) and [cli/src/bin/sessiondemo.rs](../../gomoku/crates/cli/src/bin/sessiondemo.rs).

```rust
    if let Some(resignation) = snapshot.resignation {
        lines.push(format!("{:?} resigned: {} candidates (heuristic; other moves unchecked)",
            resignation.by, resignation.candidates));
    }
```

The preceding block is for the vector-returning REPL; the demo uses `println!` with the same format and arguments instead of `lines.push`. `Snapshot.status` remains the actual `Board::status()` (`Ongoing` for a conceded position), not a fabricated color win.

In [cli/src/tui/input.rs](../../gomoku/crates/cli/src/tui/input.rs), add an early `if core.is_finished() { return InputOutcome::None; }` inside the Enter/Space arm of `playing_key`; change the empty-line guard to `if core.snapshot().opening.is_some() || core.is_finished()`. Keep its local board for the seat lookup. Move the `Status` import into the existing test module (which still asserts engine terminal status).

In `toggle_tss` in [cli/src/tui/study.rs](../../gomoku/crates/cli/src/tui/study.rs), replace the board-status guard with `if core.is_finished()` and remove the now-unused production `Status` import. In `render_study` in [cli/src/tui/render.rs](../../gomoku/crates/cli/src/tui/render.rs), pass `if core.is_finished() { None } else { view.tss_proof }` to `study_board_lines`. This also suppresses previously stored proof overlays after navigation to a conceded node. Policy/value statistics may remain visible: they describe a real search. Keep back/forward/var and all their event behavior unchanged.

### 12. Version 2: persist historical concessions

Do not add new semantics to a file still labeled version 1. After Part 2, the writer emits version 2 and the reader accepts versions 1 and 2. An installed Part-1-only reader rejects version 2. Existing version-1 records lack `AiConfig.resign` and annotations, so they load with false/None.

In `gomoku/crates/cli/src/history/record.rs`, add this DTO and add `#[serde(default)] pub resignation: Option<ResignationRecord>` to `NodeRecord`.

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum ResignationRecord {
    VerifiedOpponentVcf { by: Color, candidates: usize },
}
```

In `capture`, add this field to its `NodeRecord` literal in `gomoku/crates/cli/src/history/record.rs`.

```rust
                resignation: node.resignation.map(|r| ResignationRecord::VerifiedOpponentVcf {
                    by: r.by, candidates: r.candidates,
                }),
```

In `restore`, insert this second pass immediately before its final `history.cursor = self.cursor` in `gomoku/crates/cli/src/history/record.rs`.

```rust
        // Replay the legal board tree first; concessions never delete children.
        for (index, node) in self.nodes.iter().enumerate() {
            if let Some(ResignationRecord::VerifiedOpponentVcf { by, candidates }) = &node.resignation {
                history.cursor = index as u32;
                let board = history.board();
                let cap = board.empty_moves().count().min(5);
                if board.status() != engine::Status::Ongoing || *by != board.to_move()
                    || *candidates == 0 || *candidates > cap
                {
                    return Err(format!("node {index}: invalid resignation annotation"));
                }
                history.set_resignation(super::Resignation { by: *by, candidates: *candidates });
            }
        }
```

This validates a **historical concession record**, not a replayable proof certificate: load performs no TSS calls. Known reason, ongoing board, matching side, and count `1..=min(5, legal_count)` are required. Existing children are valid study continuations, not evidence that the concession must be deleted.

In `gomoku/crates/cli/src/session/record.rs`, change the writer's `version: 1` to `version: 2`, change the reader's version predicate to `!matches!(record.version, 1 | 2)`, and insert this version-1-specific check before restoring history.

```rust
        if record.version == 1 {
            let seat_resigns = |occupant: &Occupant| {
                matches!(occupant, Occupant::Ai(config) if config.resign)
            };
            if record.default_config.resign || seat_resigns(&record.black)
                || seat_resigns(&record.white)
                || record.history.nodes.iter().any(|node| node.resignation.is_some())
            {
                return Err("version 1 cannot contain resignation semantics".into());
            }
        }
```

Version-2 node JSON for a Black concession is `{"parent":null,"square":null,"resignation":{"reason":"verified_opponent_vcf","by":"Black","candidates":5}}`; put it on a genuinely ongoing Black-to-move root. Null/missing annotation means none; unknown reason strings fail serde. Seat/default configurations now carry `"resign": false` or `true`. Do not overwrite or reinterpret any existing saved diagnostic artifact as a version-2 native study.

### 13. Focused resignation tests

Append this module to `gomoku/crates/player/src/resign.rs`; the injected boundary replaces only prover selection, never `verify_line`.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn mv(row: u8, col: u8) -> Move { Move::new(row, col).unwrap() }

    fn losing_position() -> Board {
        Board::from_position(
            &[mv(2, 0), mv(2, 2), mv(2, 4), mv(2, 6)],
            &[mv(7, 3), mv(7, 4), mv(7, 5), mv(7, 6)], Color::Black,
        ).unwrap()
    }

    fn edges(board: &Board) -> Vec<Edge> {
        board.empty_moves().map(|mv| Edge {
            mv, prior: 0.0, n: 0, w: 0.0, child: None,
        }).collect()
    }

    fn white_certificate(after: &Board) -> Proof {
        let first = [mv(7, 2), mv(7, 7)].into_iter().find(|&m| after.is_legal(m)).unwrap();
        let proof = Proof { winner: Color::White, line: vec![first] };
        assert!(engine::verify_line(after, &proof));
        proof
    }

    #[test]
    fn five_real_verified_refutations_and_stable_ties() {
        let board = losing_position();
        let original = board.clone();
        let mut ranked = edges(&board);
        for (square, n, prior) in [
            (mv(0, 4), 9, 0.1), (mv(0, 3), 8, 0.1),
            (mv(0, 2), 7, 0.9), (mv(0, 1), 7, 0.9), (mv(0, 0), 7, 0.5),
        ] {
            let edge = ranked.iter_mut().find(|edge| edge.mv == square).unwrap();
            edge.n = n;
            edge.prior = prior;
        }
        ranked.reverse();
        let mut checked = Vec::new();
        let result = consider(&board, &ranked, |after, side, budget| {
            assert_eq!(side, Color::White);
            assert_eq!(budget, GuardConfig::baseline().budget);
            checked.push(*after.moves().last().unwrap());
            Some(white_certificate(after))
        });
        assert_eq!(checked, vec![mv(0, 4), mv(0, 3), mv(0, 1), mv(0, 2), mv(0, 0)]);
        assert_eq!(result, Some(ResignReason::VerifiedOpponentVcf { candidates: 5 }));
        assert_eq!(board, original);
    }

    #[test]
    fn unknown_or_malformed_certificate_cancels_concession() {
        let board = losing_position();
        let edges = edges(&board);
        for mode in 0..6 {
            let mut calls = 0;
            let result = consider(&board, &edges, |after, _, _| {
                calls += 1;
                if calls != 3 { return Some(white_certificate(after)); }
                match mode {
                    0 => None,
                    1 => Some(Proof { winner: Color::Black, line: vec![mv(7, 2)] }),
                    2 => Some(Proof { winner: Color::White, line: vec![] }),
                    3 => Some(Proof { winner: Color::White, line: vec![mv(7, 3)] }),
                    4 => Some(Proof { winner: Color::White, line: vec![mv(0, 10), mv(0, 11)] }),
                    _ => Some(Proof { winner: Color::White,
                        line: vec![mv(7, 2), mv(14, 14)] }),
                }
            });
            assert_eq!(result, None);
            assert_eq!(calls, 3);
        }
    }

    #[test]
    fn own_win_outside_top_five_and_incomplete_roots_prevent_resignation() {
        let board = Board::from_position(
            &[mv(14, 1), mv(14, 2), mv(14, 3), mv(14, 4)],
            &[mv(7, 3), mv(7, 4), mv(7, 5), mv(7, 6)], Color::Black,
        ).unwrap();
        assert_eq!(consider(&board, &edges(&board), |_, _, _| panic!("own win")), None);
        let losing = losing_position();
        let mut incomplete = edges(&losing);
        incomplete.pop();
        assert_eq!(consider(&losing, &incomplete, |_, _, _| panic!("incomplete")), None);
        let mut duplicate = edges(&losing);
        duplicate[1].mv = duplicate[0].mv;
        assert_eq!(consider(&losing, &duplicate, |_, _, _| panic!("duplicate")), None);
    }
}
```

Add this test inside the existing tests of [player/src/lib.rs](../../gomoku/crates/player/src/lib.rs), where `SearchStats::from_searched` is visible.

```rust
    #[test]
    fn resignation_has_real_root_statistics_and_consumes_no_selection_rng() {
        use rand::{RngExt, SeedableRng, rngs::StdRng};
        let mv = |r, c| engine::Move::new(r, c).unwrap();
        let board = Board::from_position(
            &[mv(2, 0), mv(2, 2), mv(2, 4), mv(2, 6)],
            &[mv(7, 3), mv(7, 4), mv(7, 5), mv(7, 6)], engine::Color::Black,
        ).unwrap();
        let options = PlayerOptions { simulations: 7, resign: true,
            temperature: 1.0, noise_epsilon: 0.25, ..PlayerOptions::default() };
        let config = SearchConfig { simulations: 7, c_puct: options.c_puct };
        let mut manual = mcts::UniformEvaluator::new(0.0);
        let searched = mcts::search(&board, &mut manual, &config);
        let expected = SearchStats::from_searched(&searched.tree, searched.root, &board);
        let mut ai = AiPlayer::new(Box::new(mcts::UniformEvaluator::new(0.0)));
        let mut rng = StdRng::seed_from_u64(77);
        let mut untouched = StdRng::seed_from_u64(77);
        let decision = ai.decide(&board, &options, &mut rng).unwrap();
        assert_eq!(decision.action, DecisionAction::Resign(
            ResignReason::VerifiedOpponentVcf { candidates: 5 }));
        assert!(decision.guard.is_none());
        assert_eq!(decision.search, Some(expected));
        assert_eq!(rng.random::<u64>(), untouched.random::<u64>());
    }

    #[test]
    fn unknown_resignation_probe_preserves_selection_and_rng() {
        use rand::{RngExt, SeedableRng, rngs::StdRng};
        let mut board = Board::new();
        board.play(engine::Move::new(7, 7).unwrap()).unwrap();
        let ordinary = PlayerOptions { simulations: 3, temperature: 1.0,
            noise_epsilon: 0.25, ..PlayerOptions::default() };
        let enabled = PlayerOptions { resign: true, ..ordinary };
        let mut a = AiPlayer::new(Box::new(mcts::UniformEvaluator::new(0.0)));
        let mut b = AiPlayer::new(Box::new(mcts::UniformEvaluator::new(0.0)));
        let mut rng_a = StdRng::seed_from_u64(19);
        let mut rng_b = StdRng::seed_from_u64(19);
        let previous = a.decide(&board, &ordinary, &mut rng_a).unwrap();
        let fallback = b.decide(&board, &enabled, &mut rng_b).unwrap();
        assert_eq!(fallback, previous);
        assert!(matches!(fallback.action, DecisionAction::Play(_)));
        assert_eq!(rng_a.random::<u64>(), rng_b.random::<u64>());
    }
```

Add these tests inside the existing tests of [cli/src/session.rs](../../gomoku/crates/cli/src/session.rs), so the concession path can be tested without a costly AI match.

```rust
    #[test]
    fn resignation_annotates_current_node_and_navigation_restores_it() {
        let mut core = SessionCore::new(SessionInit::hotseat());
        let first = Move::new(7, 7).unwrap();
        core.execute(Command::Play { mv: first }).unwrap();
        core.execute(Command::Play { mv: Move::new(7, 8).unwrap() }).unwrap();
        core.execute(Command::Back { n: 1 }).unwrap();
        let before = core.snapshot();
        let board = core.board();
        let stats = player::SearchStats { side_to_move: board.to_move(),
            visits: vec![(Move::new(6, 6).unwrap(), 3)],
            prior: vec![(Move::new(6, 6).unwrap(), 1.0)], value: -0.5 };
        let config = core.default_config().clone();
        let events = core.record_resignation(
            player::ResignReason::VerifiedOpponentVcf { candidates: 5 }, stats, &config);
        assert_eq!(events, vec![Event::Resigned { by: Color::White, candidates: 5 }]);
        assert_eq!(core.board(), board);
        assert_eq!(core.snapshot().ply, before.ply);
        assert_eq!(core.snapshot().last_move, before.last_move);
        assert_eq!(core.snapshot().status, Status::Ongoing);
        assert!(core.is_finished());
        assert_eq!(core.search_info().unwrap().side_to_move, Color::White);
        assert_eq!(core.execute(Command::Step), Err(CommandError::GameOver));
        assert_eq!(core.execute(Command::Play { mv: Move::new(6, 6).unwrap() }),
            Err(CommandError::GameOver));
        core.execute(Command::Back { n: 1 }).unwrap();
        assert!(!core.is_finished());
        core.execute(Command::Forward { n: 1 }).unwrap();
        assert!(core.is_finished());
        core.execute(Command::Forward { n: 1 }).unwrap();
        assert!(!core.is_finished());
        core.execute(Command::Back { n: 1 }).unwrap();
        assert!(core.is_finished());
        core.execute(Command::New).unwrap();
        assert!(core.snapshot().resignation.is_none());
    }
```

Add this compatibility test to `gomoku/crates/cli/src/session/record.rs` after applying the version-2 delta.

```rust
    #[test]
    fn v1_defaults_and_v2_annotation_survive_without_reproving() {
        let mut core = SessionCore::new(SessionInit::hotseat());
        core.history.set_resignation(crate::history::Resignation {
            by: engine::Color::Black, candidates: 5,
        });
        let text = core.export_record_json().unwrap();
        let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["version"], 2);
        let restored = SessionCore::import_record_json(&text).unwrap();
        assert_eq!(restored.snapshot().resignation, core.snapshot().resignation);
        value["version"] = 1.into();
        assert!(SessionCore::import_record_json(&value.to_string()).is_err());
        value["history"]["nodes"][0].as_object_mut().unwrap().remove("resignation");
        value["default_config"].as_object_mut().unwrap().remove("resign");
        let old = SessionCore::import_record_json(&value.to_string()).unwrap();
        assert!(!old.default_config().resign);
        assert!(old.snapshot().resignation.is_none());
        value["version"] = 3.into();
        assert!(SessionCore::import_record_json(&value.to_string()).is_err());
    }
```

Remaining focused cases: fewer than five legal candidates, imported final-cell draw never resigning, wrong-side/count/terminal annotations, unknown reason, version-1 AI-seat missing `resign`, seeded disabled-path equivalence, enabled-but-unknown RNG equivalence, own-proof bypass despite opt-in, Enter/Space/empty line after concession, TSS suppression, snapshot serialization with both optional fields and with neither, and var navigation through already-existing siblings. Reuse small deterministic fixtures; no new 10,000-simulation debug test or ignored acceptance run is needed.

### 14. Integration inventory and gates

Apply these mechanical migrations with the feature implementation, not to the existing working tree while reading this solution:

- [cli/src/lib.rs](../../gomoku/crates/cli/src/lib.rs): register `game_file` and `shell_command`; history/session register child record modules.
- [cli/src/command.rs](../../gomoku/crates/cli/src/command.rs): both path variants, early path grammar, shell-only error, concession event/display; audit exhaustive matches.
- [cli/src/session.rs](../../gomoku/crates/cli/src/session.rs): record module, effective finish, action dispatch, concession storage, snapshot field plus both manual serde halves; preserve ordinary child-node search storage and reset RNG behavior.
- [cli/src/actor.rs](../../gomoku/crates/cli/src/actor.rs) and [cli/src/main.rs](../../gomoku/crates/cli/src/main.rs): false-default opt-in, flag, config copy, explicit options and test literal migration.
- [cli/src/repl.rs](../../gomoku/crates/cli/src/repl.rs), [cli/src/bin/sessiondemo.rs](../../gomoku/crates/cli/src/bin/sessiondemo.rs), [cli/src/tui/study.rs](../../gomoku/crates/cli/src/tui/study.rs): shared shell execution, help, load-only view reset, effective outcome reporting.
- [cli/src/tui/input.rs](../../gomoku/crates/cli/src/tui/input.rs) and [cli/src/tui/render.rs](../../gomoku/crates/cli/src/tui/render.rs): effective-finish input/proof guards; preserve board status and navigation.
- [player/src/lib.rs](../../gomoku/crates/player/src/lib.rs): both decision constructors, new action/reason, optional guard, false-default option, one post-search admission block; [player/src/guard.rs](../../gomoku/crates/player/src/guard.rs) stays unchanged.
- [player/tests/decisions.rs](../../gomoku/crates/player/tests/decisions.rs) and [player/tests/guard.rs](../../gomoku/crates/player/tests/guard.rs): replace old move assertions with `DecisionAction::Play(expected)`, destructure Play before legality/play calls, and use `decision.guard.as_ref().expect("play has diagnostics")` for old guard assertions. Preserve all previous expected moves, refutations, and RNG assertions.
- [cli/tests/actor.rs](../../gomoku/crates/cli/tests/actor.rs), [cli/tests/acceptance.rs](../../gomoku/crates/cli/tests/acceptance.rs), [cli/tests/session.rs](../../gomoku/crates/cli/tests/session.rs), [cli/tests/study.rs](../../gomoku/crates/cli/tests/study.rs), and internal session/overlay tests: add `resign: false` only to exhaustive `AiConfig` literals; struct-update literals inherit it. Add `resignation: None` to exhaustive snapshot literals. Existing acceptance loops retain disabled resignation, or use `core.is_finished()` when explicitly testing the new opt-in.
- [testkit/src/lib.rs](../../gomoku/crates/testkit/src/lib.rs): add `resign: false` to both exhaustive `AiConfig` constructors, `ai_config_tactics` and `default_ai_config`, preserving their previous deterministic defaults.
- [baseline/tests/acceptance.rs](../../gomoku/crates/baseline/tests/acceptance.rs): migrate `best_move`, the baseline branch of `play_game`, and the latency acceptance loop from `.mv` to explicit `DecisionAction::Play(mv)` matching, rejecting unexpected Resign in these disabled-option tests. Keep `BaselineMetrics::record`, requested/actual simulation accounting, existing fork-bonus code, and saved acceptance results unchanged.
- [cli/examples/diagnose_g4.rs](../../gomoku/crates/cli/examples/diagnose_g4.rs) also uses old `result.mv` and mandatory guard fields in controlled-guard and real-decision output. Keep its default `resign: false`, explicitly destructure `DecisionAction::Play(mv)` and borrow `result.guard.as_ref().expect("resignation disabled")`; preserve the existing output schema and factual labels. Do not rewrite saved outputs to resemble a new run.

Recommended implementation-time gates from `gomoku/`, not executed for this document:

```bash
cargo test --release --locked --all-targets
cargo test --release --locked -p cli --no-default-features --all-targets
cargo check --locked -p engine
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --all -- --check
```

The feature-disabled CLI gate must exercise rejection of Baseline in either seat **and in default_config** before construction. None of these proposed snippets or tests has been compiled here; source inspection is not a green test result.

## References

- [History arena](../../gomoku/crates/cli/src/history.rs), [session state machine](../../gomoku/crates/cli/src/session.rs), [actor serde types](../../gomoku/crates/cli/src/actor.rs), and [command grammar](../../gomoku/crates/cli/src/command.rs), inspected at the stated source baseline.
- [Player decisions](../../gomoku/crates/player/src/lib.rs) and [guard implementation and fixtures](../../gomoku/crates/player/src/guard.rs): existing prover budgets, verified-proof admission, ranking and selection pipeline.
- [Engine board](../../gomoku/crates/engine/src/board.rs) and [proof verifier](../../gomoku/crates/engine/src/tss.rs): imported-root semantics, legality, terminal status and certificate validation.
