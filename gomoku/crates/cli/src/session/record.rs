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
            return Err("save/load unavailable during Swap2 ceremony".into());
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
            return Err("study exceeds 4 MiB.".into());
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