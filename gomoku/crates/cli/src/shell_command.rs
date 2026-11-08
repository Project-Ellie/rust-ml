use crate::command::{Command, Event};
use crate::game_file;
use crate::session::SessionCore;

/// Events and shell feedback from one successful command
#[derive(Debug, Default)]
pub struct SharedShellReply {
    /// Ordinary core events; save/load do not fabricate game events.
    pub events: Vec<Event>,
    /// Optional file system operation feedback.
    pub message: Option<String>,
    /// True only after a successful replacement load
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
        other => core
            .execute(other)
            .map(|events| SharedShellReply {
                events,
                ..SharedShellReply::default()
            })
            .map_err(|e| e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SessionInit;
    use engine::Move;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn scratch() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "gomoku-record-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        path
    }

    #[test]
    fn resign_load_preserves_annotations_and_invalid_hidden_node_rolls_back() {
        let dir = scratch();
        let path = dir.join("Conceded Study.json");
        let mut source = SessionCore::new(SessionInit::hotseat());
        source
            .execute(Command::Play {
                mv: Move::new(7, 7).unwrap(),
            })
            .unwrap();
        source
            .execute(Command::Play {
                mv: Move::new(7, 8).unwrap(),
            })
            .unwrap();
        source.execute(Command::Back { n: 2 }).unwrap();
        let mut value: serde_json::Value =
            serde_json::from_str(&source.export_record_json().unwrap()).unwrap();
        value["version"] = 2.into();
        value["history"]["nodes"][0]["resignation"] = serde_json::json!({
            "reason": "verified_opponent_vcf", "by": "Black", "candidates": 5
        });
        value["history"]["nodes"][1]["resignation"] = serde_json::json!({
            "reason": "verified_opponent_vcf", "by": "White", "candidates": 4
        });
        std::fs::write(&path, value.to_string()).unwrap();
        let mut core = SessionCore::new(SessionInit::hotseat());
        let reply = execute(&mut core, Command::Load { path: path.clone() }).unwrap();
        assert!(reply.loaded && reply.events.is_empty());
        assert!(core.is_finished());
        core.execute(Command::Forward { n: 1 }).unwrap();
        assert_eq!(
            core.snapshot().resignation.unwrap().by,
            engine::Color::White
        );
        core.execute(Command::Forward { n: 1 }).unwrap();
        assert!(!core.is_finished());
        core.execute(Command::Back { n: 2 }).unwrap();
        let before = core.export_record_json().unwrap();
        let snapshot = core.snapshot();
        value["history"]["nodes"][1]["resignation"]["by"] = "Black".into();
        std::fs::write(&path, value.to_string()).unwrap();
        assert!(execute(&mut core, Command::Load { path: path.clone() }).is_err());
        assert_eq!(core.export_record_json().unwrap(), before);
        assert_eq!(core.snapshot(), snapshot);
        execute(&mut core, Command::Save { path: path.clone() }).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn failed_load_rolls_back_and_pending_swap2_refuses_both_commands() {
        let dir = scratch();
        let path = dir.join("Broken Study.json");
        std::fs::write(&path, b"{broken").unwrap();
        let mut core = SessionCore::new(SessionInit::hotseat());
        core.execute(Command::Play {
            mv: Move::new(7, 7).unwrap(),
        })
        .unwrap();
        let before = core.export_record_json().unwrap();
        assert!(execute(&mut core, Command::Load { path: path.clone() }).is_err());
        assert_eq!(core.export_record_json().unwrap(), before);
        let mut pending = SessionCore::new(SessionInit::hotseat().with_swap2(true));
        let snapshot = pending.snapshot();
        for cmd in [
            Command::Save { path: path.clone() },
            Command::Load { path: path.clone() },
        ] {
            assert!(execute(&mut pending, cmd).is_err());
            assert_eq!(pending.snapshot(), snapshot);
        }
        assert_eq!(std::fs::read(&path).unwrap(), b"{broken");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
