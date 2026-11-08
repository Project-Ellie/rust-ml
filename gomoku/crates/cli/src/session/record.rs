//! Native version-3 study records with version-1/2 compatibility; pure validation.
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

pub(crate) fn validate_config_values(config: &AiConfig) -> Result<(), String> {
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
    Ok(())
}

fn validate_config(config: &AiConfig) -> Result<(), String> {
    validate_config_values(config)?;
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
            version: 3,
            history: HistoryRecord::capture(&self.history),
            black: self.black.clone(),
            white: self.white.clone(),
            default_config: self.default_config.clone(),
            black_sims: self.black_sims,
            white_sims: self.white_sims,
            seed: self.init.seed,
        };

        record.validate_configs()?;
        record.history.validate_version(3)?;
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
        if record.format != "gomoku-study" || !matches!(record.version, 1..=3) {
            return Err("unsupported study format/version".into());
        }
        if record.version == 1 {
            let seat_resigns =
                |seat: &Occupant| matches!(seat, Occupant::Ai(config) if config.resign);
            if record.default_config.resign
                || seat_resigns(&record.black)
                || seat_resigns(&record.white)
                || record
                    .history
                    .nodes
                    .iter()
                    .any(|node| node.resignation.is_some())
            {
                return Err("version 1 cannot contain resignation semantics".into());
            }
        }
        record.validate_configs()?;
        let history = record.history.restore(record.version)?;
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

    fn real_concession(kind: EvaluatorKind) -> SessionCore {
        use engine::{Board, Color};
        let mv = |row, col| Move::new(row, col).unwrap();
        let board = Board::from_position(
            &[mv(2, 0), mv(2, 2), mv(2, 4), mv(2, 6)],
            &[mv(7, 3), mv(7, 4), mv(7, 5), mv(7, 6)],
            Color::Black,
        )
        .unwrap();
        let config = AiConfig {
            evaluator: kind,
            simulations: 3,
            resign: true,
            ..AiConfig::default()
        };
        let mut core = SessionCore::new(SessionInit {
            opening: board,
            seed: Some(42),
            ..SessionInit::versus_ai(Color::White, config)
        });
        core.execute(Command::Step).unwrap();
        assert!(core.is_finished());
        core
    }

    #[test]
    fn resign_original_audits_and_root_stats_survive_active_and_resumed_roundtrip() {
        use crate::command::SeatSel;
        for kind in [EvaluatorKind::Tactics, EvaluatorKind::Baseline] {
            if kind == EvaluatorKind::Baseline && !cfg!(feature = "baseline") {
                continue;
            }
            let mut core = real_concession(kind);
            for resumed in [false, true] {
                if resumed {
                    core.execute_take(SeatSel::Black).unwrap();
                }
                let text = core.export_record_json().unwrap();
                let loaded = SessionCore::import_record_json(&text).unwrap();
                assert_eq!(loaded.resignation_events(), core.resignation_events());
                assert_eq!(loaded.search_info(), core.search_info());
                assert_eq!(loaded.is_finished(), !resumed);
                assert_eq!(loaded.snapshot().resignation_resumed, resumed);
                assert_eq!(
                    loaded.resignation_events()[0]
                        .audit
                        .as_ref()
                        .unwrap()
                        .evidence
                        .candidates
                        .len(),
                    5
                );
            }
        }
    }

    #[test]
    fn resign_resume_release_appends_independent_audits_and_preserves_descendants() {
        use crate::command::{Event, SeatSel};
        use engine::{Color, Status};
        let mut core = real_concession(EvaluatorKind::Tactics);
        let original = core.resignation_events()[0].clone();
        let board = core.board();
        assert_eq!(core.execute_take(SeatSel::White).unwrap().len(), 1);
        assert!(core.is_finished());
        let events = core.execute_take(SeatSel::Both).unwrap();
        assert_eq!(events.last(), Some(&Event::Resumed { by: Color::Black }));
        assert_eq!(events.len(), 3);
        assert_eq!(core.execute_take(SeatSel::Black).unwrap().len(), 1);
        assert_eq!(core.snapshot().status, Status::Ongoing);
        assert_eq!(core.board(), board);
        core.execute_play(Move::new(0, 0).unwrap()).unwrap();
        core.execute_back(1).unwrap();
        core.execute_release(SeatSel::Black).unwrap();
        assert!(!core.is_finished()); // Release never reactivates old concessions.
        core.execute_set_sims(SeatSel::Black, 4).unwrap();
        core.execute(Command::Step).unwrap();
        assert_eq!(core.resignation_events().len(), 2);
        let mut resumed_original = original;
        resumed_original.resumed = true;
        assert_eq!(core.resignation_events()[0], resumed_original);
        assert!(!core.resignation_events()[1].resumed);
        assert_eq!(core.search_info().unwrap().config.simulations, 4);
        let text = core.export_record_json().unwrap();
        let mut loaded = SessionCore::import_record_json(&text).unwrap();
        assert_eq!(loaded.resignation_events(), core.resignation_events());
        loaded.execute_forward(1).unwrap();
        assert!(!loaded.is_finished());
        loaded.execute_back(1).unwrap();
        assert!(loaded.is_finished());
        loaded.execute_take(SeatSel::Black).unwrap();
        loaded.execute_play(Move::new(0, 1).unwrap()).unwrap();
        loaded.execute_back(1).unwrap();
        assert_eq!(loaded.snapshot().branch_options.len(), 2);
        loaded.execute_new().unwrap();
        assert!(loaded.resignation_events().is_empty());
    }

    #[test]
    fn resign_v2_legacy_is_active_and_cannot_invent_audits() {
        let core = SessionCore::new(SessionInit::hotseat());
        let mut value: serde_json::Value =
            serde_json::from_str(&core.export_record_json().unwrap()).unwrap();
        value["version"] = 2.into();
        value["history"]["nodes"][0]["resignation"] = serde_json::json!({
            "reason": "verified_opponent_vcf", "by": "Black", "candidates": 5
        });
        let mut loaded = SessionCore::import_record_json(&value.to_string()).unwrap();
        assert!(loaded.is_finished());
        assert!(loaded.resignation_events()[0].audit.is_none());
        assert!(loaded.search_info().is_none());
        loaded.execute_take(crate::command::SeatSel::Black).unwrap();
        let v3 = loaded.export_record_json().unwrap();
        let again = SessionCore::import_record_json(&v3).unwrap();
        assert_eq!(again.resignation_events(), loaded.resignation_events());
        assert!(!again.is_finished());
        let mut unknown: serde_json::Value = serde_json::from_str(&v3).unwrap();
        unknown["history"]["nodes"][0]["resignations"][0]["evidence"]["fabricated"] = true.into();
        assert!(SessionCore::import_record_json(&unknown.to_string()).is_err());
        for version in [1, 3] {
            value["version"] = version.into();
            assert!(SessionCore::import_record_json(&value.to_string()).is_err());
        }
    }

    #[test]
    fn resign_hidden_original_corruption_matrix_rejects_and_shell_load_rolls_back() {
        use serde_json::{Value, json};
        let mut source = real_concession(EvaluatorKind::Tactics);
        source.execute_take(crate::command::SeatSel::Black).unwrap();
        source.execute_play(Move::new(0, 0).unwrap()).unwrap();
        let value: Value = serde_json::from_str(&source.export_record_json().unwrap()).unwrap();
        // The audited node is hidden: cursor is its child.
        assert_eq!(value["history"]["cursor"], 1);
        let prefix = "/history/nodes/0/resignations/0";
        let cases = [
            ("/by", json!("White")),
            ("/candidates", json!(0)),
            ("/reason", json!("value_threshold")),
            ("/resumed", Value::Null),
            ("/evidence/kind", json!("regenerated")),
            ("/evidence/candidates/0/ordinal", json!(2)),
            ("/evidence/candidates/0/square", json!([15, 0])),
            ("/evidence/candidates/0/visits", json!(17)),
            ("/evidence/candidates/0/prior", json!(0.99)),
            ("/evidence/candidates/0/probe_budget/max_nodes", json!(1001)),
            ("/evidence/candidates/0/probe_budget/max_depth", json!(11)),
            ("/evidence/candidates/0/proof/winner", json!("Black")),
            ("/evidence/candidates/0/proof/line", json!([[9, 9]])),
            ("/evidence/candidates/0/proof/line", json!([[255, 255]])),
            ("/evidence/candidates/0/proof/line", json!([])),
            ("/evidence/candidates/0/proof/line", json!([[7, 7], [0, 0]])),
            ("/evidence/candidates/0/proof/line", json!(vec![[0, 0]; 11])),
            ("/evidence/search/side_to_move", json!("White")),
            ("/evidence/search/value", json!(2.0)),
            ("/evidence/search/value", json!("NaN")),
            ("/evidence/search/edges/0/square", json!([2, 0])),
            ("/evidence/search/edges/0/prior", json!(-0.1)),
            ("/evidence/search/edges/0/prior", json!(0.5)),
            ("/evidence/search/edges/0/visits", json!(99)),
            ("/evidence/search/config/simulations", json!(0)),
            ("/evidence/search/config/temperature", json!(-1.0)),
            ("/evidence/search/config/c_puct", json!(-1.0)),
            ("/evidence/search/config/noise_epsilon", json!(1.1)),
            ("/evidence/search/config/resign", json!(false)),
            ("/evidence/options/simulations", json!(4)),
            ("/evidence/options/noise_alpha", json!(0.0)),
            ("/evidence/options/resign", json!(false)),
            ("/evidence/options/own/enabled", json!(true)),
            ("/evidence/options/own/budget/max_depth", json!(29)),
            ("/evidence/options/guard/max_candidates", json!(4)),
            ("/evidence/options/guard/budget/max_nodes", json!(999)),
        ];
        let mut invalids: Vec<(String, Value)> = cases
            .into_iter()
            .map(|(path, replacement)| {
                let mut invalid = value.clone();
                *invalid.pointer_mut(&format!("{prefix}{path}")).unwrap() = replacement;
                (path.to_string(), invalid)
            })
            .collect();
        for path in [
            "",
            "/evidence",
            "/evidence/options",
            "/evidence/options/own",
            "/evidence/options/guard/budget",
            "/evidence/search",
            "/evidence/search/config",
            "/evidence/search/edges/0",
            "/evidence/candidates/0",
            "/evidence/candidates/0/proof",
        ] {
            let mut invalid = value.clone();
            invalid
                .pointer_mut(&format!("{prefix}{path}"))
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("unknown".into(), json!(true));
            invalids.push((format!("unknown {path}"), invalid));
        }
        for mode in [
            "duplicate_candidate",
            "rank_order",
            "short_count",
            "missing_edge",
            "duplicate_edge",
            "too_many_edges",
            "resume_order",
            "missing_resumed",
            "old_version",
        ] {
            let mut invalid = value.clone();
            let event = invalid.pointer_mut(prefix).unwrap();
            match mode {
                "duplicate_candidate" => {
                    event["evidence"]["candidates"][1] = event["evidence"]["candidates"][0].clone()
                }
                "rank_order" => {
                    event["evidence"]["candidates"]
                        .as_array_mut()
                        .unwrap()
                        .swap(0, 1);
                    event["evidence"]["candidates"][0]["ordinal"] = json!(1);
                    event["evidence"]["candidates"][1]["ordinal"] = json!(2);
                }
                "short_count" => {
                    event["candidates"] = json!(4);
                    event["evidence"]["candidates"]
                        .as_array_mut()
                        .unwrap()
                        .pop();
                }
                "missing_edge" => {
                    event["evidence"]["search"]["edges"]
                        .as_array_mut()
                        .unwrap()
                        .pop();
                }
                "duplicate_edge" => {
                    event["evidence"]["search"]["edges"][1] =
                        event["evidence"]["search"]["edges"][0].clone()
                }
                "too_many_edges" => {
                    event["evidence"]["search"]["edges"] =
                        json!(vec![event["evidence"]["search"]["edges"][0].clone(); 226])
                }
                "resume_order" => {
                    event["resumed"] = json!(false);
                    let repeated = event.clone();
                    invalid["history"]["nodes"][0]["resignations"]
                        .as_array_mut()
                        .unwrap()
                        .push(repeated);
                }
                "missing_resumed" => {
                    event.as_object_mut().unwrap().remove("resumed");
                }
                "old_version" => invalid["version"] = json!(2),
                _ => unreachable!(),
            }
            invalids.push((mode.to_string(), invalid));
        }
        let mut own_win = value.clone();
        own_win["history"]["root"]["black"] = json!([[2, 0], [2, 1], [2, 2], [2, 3]]);
        invalids.push(("root own immediate-win veto".into(), own_win));
        let path =
            std::env::temp_dir().join(format!("gomoku-slice2-corrupt-{}.json", std::process::id()));
        let before = source.export_record_json().unwrap();
        let snapshot = source.snapshot();
        let mut rng = StdRng::seed_from_u64(42);
        for (name, invalid) in invalids {
            std::fs::write(&path, invalid.to_string()).unwrap();
            assert!(
                crate::shell_command::execute(&mut source, Command::Load { path: path.clone() })
                    .is_err(),
                "{name}"
            );
            assert_eq!(source.export_record_json().unwrap(), before, "{name}");
            assert_eq!(source.snapshot(), snapshot, "{name}");
            assert_eq!(source.rng.random::<u64>(), rng.random::<u64>(), "{name}");
        }
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn resign_historical_evaluator_and_scheduled_effective_options_need_no_live_feature() {
        let core = real_concession(EvaluatorKind::Tactics);
        let mut value: serde_json::Value =
            serde_json::from_str(&core.export_record_json().unwrap()).unwrap();
        // Historical identity is independent of live seats/defaults. The
        // baseline and tactics presets share budgets, differing in enablement.
        value["black"] = serde_json::json!("Human");
        value["white"] = serde_json::json!("Human");
        let evidence = &mut value["history"]["nodes"][0]["resignations"][0]["evidence"];
        evidence["search"]["config"]["evaluator"] = serde_json::json!("Baseline");
        evidence["options"]["own"]["enabled"] = true.into();
        evidence["options"]["guard"]["enabled"] = true.into();
        evidence["search"]["config"]["temperature"] = 1.0.into();
        evidence["options"]["temperature"] = 1.0.into();
        evidence["search"]["config"]["noise_epsilon"] = 0.25.into();
        evidence["options"]["noise_epsilon"] = 0.25.into();
        assert_eq!(evidence["search"]["config"]["explicit_selection"], false);
        let loaded = SessionCore::import_record_json(&value.to_string()).unwrap();
        assert_eq!(
            loaded.search_info().unwrap().config.evaluator,
            EvaluatorKind::Baseline
        );
        assert_eq!(
            loaded.resignation_events()[0]
                .audit
                .as_ref()
                .unwrap()
                .evidence
                .options
                .temperature,
            1.0
        );
    }

    #[test]
    fn resign_record_event_tree_and_byte_limits_are_bounded() {
        use serde_json::json;
        let core = SessionCore::new(SessionInit::hotseat());
        let value: serde_json::Value =
            serde_json::from_str(&core.export_record_json().unwrap()).unwrap();
        let mut events = value.clone();
        events["history"]["nodes"][0]["resignations"] = json!(vec![
            json!({
                "reason":"verified_opponent_vcf", "by":"Black", "candidates":5,
                "resumed":true, "evidence":{"kind":"legacy_unavailable"}
            });
            10_001
        ]);
        assert!(
            SessionCore::import_record_json(&events.to_string())
                .err()
                .unwrap()
                .contains("too many resignation")
        );
        let mut nodes = value;
        nodes["history"]["nodes"] = json!(vec![nodes["history"]["nodes"][0].clone(); 10_001]);
        assert!(SessionCore::import_record_json(&nodes.to_string()).is_err());
        assert!(
            SessionCore::import_record_json(&" ".repeat(MAX_RECORD_BYTES + 1))
                .err()
                .unwrap()
                .contains("4 MiB")
        );
    }

    #[test]
    fn resign_writer_uses_v3_and_restores_legacy_unavailable_concession() {
        let mut core = SessionCore::new(SessionInit::hotseat());
        core.history
            .append_legacy_unavailable_resignation(crate::history::Resignation {
                by: engine::Color::Black,
                candidates: 5,
            });
        let text = core.export_record_json().unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["version"], 3);
        assert_eq!(
            value["history"]["nodes"][0]["resignations"][0]["evidence"]["kind"],
            "legacy_unavailable"
        );
        let restored = SessionCore::import_record_json(&text).unwrap();
        assert_eq!(restored.snapshot(), core.snapshot());
        assert!(restored.is_finished());
    }

    #[test]
    fn resign_v1_missing_flags_load_but_smuggled_semantics_are_rejected() {
        let config = AiConfig {
            simulations: 1,
            ..AiConfig::default()
        };
        let core = SessionCore::new(SessionInit::ai_vs_ai(config, 17));
        let mut old: serde_json::Value =
            serde_json::from_str(&core.export_record_json().unwrap()).unwrap();
        old["version"] = 1.into();
        for path in ["black", "white"] {
            old[path]["Ai"].as_object_mut().unwrap().remove("resign");
        }
        old["default_config"]
            .as_object_mut()
            .unwrap()
            .remove("resign");
        old["history"]["nodes"][0]
            .as_object_mut()
            .unwrap()
            .remove("resignation");
        let loaded = SessionCore::import_record_json(&old.to_string()).unwrap();
        assert!(!loaded.default_config().resign);
        for seat in [loaded.black(), loaded.white()] {
            assert!(matches!(seat, Occupant::Ai(config) if !config.resign));
        }
        assert!(loaded.snapshot().resignation.is_none());
        for path in ["black", "white", "default_config", "annotation"] {
            let mut smuggled = old.clone();
            match path {
                "annotation" => {
                    smuggled["history"]["nodes"][0]["resignation"] = serde_json::json!({
                        "reason": "verified_opponent_vcf", "by": "Black", "candidates": 5
                    })
                }
                "default_config" => smuggled[path]["resign"] = true.into(),
                _ => smuggled[path]["Ai"]["resign"] = true.into(),
            }
            assert!(
                SessionCore::import_record_json(&smuggled.to_string()).is_err(),
                "{path}"
            );
        }
    }

    #[test]
    fn resign_v2_rejects_invalid_hidden_annotations_unknown_reason_and_future_version() {
        let mut core = SessionCore::new(SessionInit::hotseat());
        core.execute(Command::Play {
            mv: Move::new(7, 7).unwrap(),
        })
        .unwrap();
        core.execute(Command::Play {
            mv: Move::new(7, 8).unwrap(),
        })
        .unwrap();
        core.execute(Command::Back { n: 2 }).unwrap();
        let value: serde_json::Value =
            serde_json::from_str(&core.export_record_json().unwrap()).unwrap();
        let mut value = value;
        value["version"] = 2.into();
        for (by, candidates) in [("White", 1), ("Black", 0), ("Black", 6)] {
            let mut invalid = value.clone();
            invalid["history"]["nodes"][2]["resignation"] = serde_json::json!({
                "reason": "verified_opponent_vcf", "by": by, "candidates": candidates
            });
            assert!(
                SessionCore::import_record_json(&invalid.to_string()).is_err(),
                "{by}, {candidates}"
            );
        }
        let mut unknown = value.clone();
        unknown["history"]["nodes"][2]["resignation"] = serde_json::json!({
            "reason": "unverified_value_threshold", "by": "Black", "candidates": 5
        });
        assert!(SessionCore::import_record_json(&unknown.to_string()).is_err());
        let mut future = value.clone();
        future["version"] = 4.into();
        assert!(SessionCore::import_record_json(&future.to_string()).is_err());
        let mut v1_hidden = value;
        v1_hidden["version"] = 1.into();
        v1_hidden["history"]["nodes"][2]["resignation"] = serde_json::json!({
            "reason": "verified_opponent_vcf", "by": "Black", "candidates": 5
        });
        assert!(SessionCore::import_record_json(&v1_hidden.to_string()).is_err());
    }

    #[test]
    fn resign_annotations_reject_terminal_and_counts_above_remaining_cells() {
        use engine::{Board, Color};
        let mut black = Vec::new();
        let mut white = Vec::new();
        let last = Move::new(0, 0).unwrap();
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
        let board = Board::from_position(&black, &white, Color::Black).unwrap();
        let mut core = SessionCore::new(SessionInit::hotseat_from_opening(&board));
        core.history
            .append_legacy_unavailable_resignation(crate::history::Resignation {
                by: Color::Black,
                candidates: 1,
            });
        let text = core.export_record_json().unwrap();
        assert!(
            SessionCore::import_record_json(&text)
                .unwrap()
                .is_finished()
        );
        let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
        value["history"]["nodes"][0]["resignations"][0]["candidates"] = 2.into();
        assert!(SessionCore::import_record_json(&value.to_string()).is_err());
        // Imported final-cell draws are terminal even though their move log is short.
        let mut draw = board;
        draw.play(last).unwrap();
        assert_eq!(draw.status(), engine::Status::Draw);
        let won = Board::from_position(
            &(0..5)
                .map(|col| Move::new(7, col).unwrap())
                .collect::<Vec<_>>(),
            &(0..4)
                .map(|col| Move::new(0, col * 2).unwrap())
                .collect::<Vec<_>>(),
            Color::White,
        )
        .unwrap();
        for terminal in [draw, won] {
            let mut terminal_core = SessionCore::new(SessionInit::hotseat_from_opening(&terminal));
            terminal_core.history.append_legacy_unavailable_resignation(
                crate::history::Resignation {
                    by: terminal.to_move(),
                    candidates: 1,
                },
            );
            assert!(
                SessionCore::import_record_json(&terminal_core.export_record_json().unwrap())
                    .is_err()
            );
        }
    }

    #[test]
    fn resign_terminal_annotation_is_rejected_on_hidden_node_too() {
        use engine::{Board, Color};
        let opening = Board::from_position(
            &(0..4)
                .map(|col| Move::new(7, col).unwrap())
                .collect::<Vec<_>>(),
            &(0..4)
                .map(|col| Move::new(0, col * 2).unwrap())
                .collect::<Vec<_>>(),
            Color::Black,
        )
        .unwrap();
        let mut core = SessionCore::new(SessionInit::hotseat_from_opening(&opening));
        core.execute(Command::Play {
            mv: Move::new(7, 4).unwrap(),
        })
        .unwrap();
        core.execute(Command::Back { n: 1 }).unwrap();
        let mut value: serde_json::Value =
            serde_json::from_str(&core.export_record_json().unwrap()).unwrap();
        value["version"] = 2.into();
        value["history"]["nodes"][1]["resignation"] = serde_json::json!({
            "reason": "verified_opponent_vcf", "by": "White", "candidates": 1
        });
        assert!(SessionCore::import_record_json(&value.to_string()).is_err());
    }

    #[test]
    fn resign_v2_full_tree_round_trip_preserves_flags_cursor_and_historical_annotations() {
        use engine::Color;
        let config = AiConfig {
            resign: true,
            simulations: 2,
            ..AiConfig::default()
        };
        let mut core = SessionCore::new(SessionInit::ai_vs_ai(config, 8));
        for (row, col) in [(7, 7), (7, 8), (8, 8)] {
            core.execute(Command::Play {
                mv: Move::new(row, col).unwrap(),
            })
            .unwrap();
        }
        core.execute(Command::Back { n: 2 }).unwrap();
        core.history
            .append_legacy_unavailable_resignation(crate::history::Resignation {
                by: Color::White,
                candidates: 4,
            });
        core.execute(Command::Back { n: 1 }).unwrap();
        core.execute(Command::Play {
            mv: Move::new(0, 0).unwrap(),
        })
        .unwrap();
        core.execute(Command::Back { n: 1 }).unwrap();
        core.history
            .append_legacy_unavailable_resignation(crate::history::Resignation {
                by: Color::Black,
                candidates: 5,
            });
        let text = core.export_record_json().unwrap();
        let mut restored = SessionCore::import_record_json(&text).unwrap();
        assert_eq!(restored.export_record_json().unwrap(), text);
        assert_eq!(restored.history, core.history);
        restored.execute(Command::Var { index: 0 }).unwrap();
        assert!(restored.is_finished());
        assert_eq!(restored.snapshot().resignation.unwrap().by, Color::White);
        restored.execute(Command::Forward { n: 2 }).unwrap();
        assert!(!restored.is_finished());
        assert_eq!(restored.snapshot().ply, 3);
        assert!(restored.default_config().resign);
        assert!(restored.search_info().is_none());
    }

    #[test]
    fn v1_saved_game_fixture_import_is_read_only() {
        let text = include_str!("../../tests/fixtures/native-study-v1.json");
        let value: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(value["version"], 1);
        if !cfg!(feature = "baseline") {
            assert!(
                SessionCore::import_record_json(text)
                    .err()
                    .unwrap()
                    .contains("baseline feature")
            );
            return;
        }
        let core = SessionCore::import_record_json(text).unwrap();
        assert!(!core.default_config().resign);
        assert!(core.snapshot().resignation.is_none());
        assert_eq!(
            core.history.cursor(),
            value["history"]["cursor"].as_u64().unwrap() as u32
        );
    }

    #[test]
    fn save_does_not_advance_rng_and_load_reseeds_instead_of_continuing() {
        let init = SessionInit {
            seed: Some(3477),
            ..SessionInit::hotseat()
        };
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
        source
            .execute(Command::Play {
                mv: Move::new(7, 8).unwrap(),
            })
            .unwrap();
        let text = source.export_record_json().unwrap();
        let mut loaded = SessionCore::import_record_json(&text).unwrap();
        loaded.execute(Command::New).unwrap();
        assert_eq!(loaded.snapshot().ply, 0);
        assert_eq!(
            loaded.board().stone_at(Move::new(7, 7).unwrap()),
            Some(engine::Color::Black)
        );
        assert_eq!(loaded.board().stone_at(Move::new(7, 8).unwrap()), None);
        assert!(!loaded.init.swap2);
        assert!(loaded.puzzle().is_none());
    }
}
