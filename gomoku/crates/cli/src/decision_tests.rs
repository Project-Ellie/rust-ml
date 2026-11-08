use crate::actor::{AiConfig, EvaluatorKind};
use crate::command::{Command, Event, SeatSel};
use crate::decision::{preview, report};
use crate::session::{SessionCore, SessionInit};
use engine::{Board, Color, Move};

pub(crate) fn concession() -> SessionCore {
    let mv = |r, c| Move::new(r, c).unwrap();
    let board = Board::from_position(
        &[mv(2, 0), mv(2, 2), mv(2, 4), mv(2, 6)],
        &[mv(7, 3), mv(7, 4), mv(7, 5), mv(7, 6)],
        Color::Black,
    )
    .unwrap();
    let mut core = SessionCore::new(SessionInit {
        opening: board,
        seed: Some(42),
        ..SessionInit::versus_ai(
            Color::White,
            AiConfig {
                simulations: 3,
                evaluator: EvaluatorKind::Tactics,
                resign: true,
                ..AiConfig::default()
            },
        )
    });
    assert!(matches!(
        core.execute(Command::Step).unwrap().as_slice(),
        [Event::Resigned { candidates: 5, .. }]
    ));
    core
}

#[test]
fn decision_original_report_preview_and_repl_are_read_only() {
    let mut core = concession();
    let before = core.export_record_json().unwrap();
    let snapshot = core.snapshot();
    let stats = core.search_info().cloned();
    let board = core.board();
    let revision = core.analysis_revision();
    for _ in 0..20 {
        core.execute(Command::Decision {
            event: None,
            candidate: None,
        })
        .unwrap();
        let text = report(&core, None, None).unwrap().join("\n");
        for n in 1..=5 {
            assert!(text.contains(&format!("Check {n}:")));
            let p = preview(&core, None, n).unwrap();
            let candidate = &core.resignation_events()[0]
                .audit
                .as_ref()
                .unwrap()
                .evidence
                .candidates[n - 1];
            let mut expected = board.clone();
            expected.play(candidate.mv).unwrap();
            assert_eq!(p.board, expected);
            assert_eq!(p.proof.winner, Color::White);
            assert_eq!(p.proof, &candidate.proof);
            assert!(p.label.contains("HYPOTHETICAL PREVIEW; no move played"));
        }
        assert!(text.contains("actual total visits 3 vs requested simulations 3"));
        assert!(text.contains("Pre-noise"));
        let repl = crate::repl::process_line("decision 1", &mut core, false)
            .lines
            .join("\n");
        assert!(repl.contains("Check 5:"));
        assert!(repl.contains("HYPOTHETICAL PREVIEW"));
    }
    assert_eq!(core.export_record_json().unwrap(), before);
    assert_eq!(core.snapshot(), snapshot);
    assert_eq!(core.search_info().cloned(), stats);
    assert_eq!(core.board(), board);
    assert_eq!(core.analysis_revision(), revision);
    core.execute(Command::Take {
        seat: SeatSel::Black,
    })
    .unwrap();
    assert!(
        report(&core, None, None)
            .unwrap()
            .join("\n")
            .contains("historical/resumed")
    );
    core.execute(Command::Play {
        mv: Move::new(0, 0).unwrap(),
    })
    .unwrap();
    core.execute(Command::Back { n: 1 }).unwrap();
    assert_eq!(core.resignation_events().len(), 1);
}

#[test]
fn decision_repeat_events_preserve_original_settings_across_v3() {
    let mut core = concession();
    core.execute(Command::Take {
        seat: SeatSel::Black,
    })
    .unwrap();
    core.execute(Command::Release {
        seat: SeatSel::Black,
    })
    .unwrap();
    core.execute(Command::SetSims {
        seat: SeatSel::Black,
        n: 4,
    })
    .unwrap();
    core.execute(Command::Step).unwrap();
    assert_eq!(core.resignation_events().len(), 2);
    assert!(
        report(&core, Some(1), None)
            .unwrap()
            .join("\n")
            .contains("requested simulations 3")
    );
    assert!(
        report(&core, Some(2), None)
            .unwrap()
            .join("\n")
            .contains("requested simulations 4")
    );
    let loaded = SessionCore::import_record_json(&core.export_record_json().unwrap()).unwrap();
    assert_eq!(report(&core, None, None), report(&loaded, None, None));
    for e in 1..=2 {
        let text = report(&core, Some(e), None).unwrap().join("\n");
        let audit = core.resignation_events()[e - 1].audit.as_ref().unwrap();
        for c in &audit.evidence.candidates {
            assert!(text.contains(&format!(
                "Check {}: candidate {} by Black; visits {}; prior {}",
                c.ordinal,
                crate::notation::algebraic(c.mv),
                c.visits,
                c.prior
            )));
        }
        assert!(text.contains(&format!("Original AI config: {:?}", audit.search.config)));
        for n in 1..=5 {
            assert_eq!(
                preview(&core, Some(e), n).unwrap().proof,
                preview(&loaded, Some(e), n).unwrap().proof
            );
        }
    }
    for (event, candidate) in [
        (Some(0), None),
        (Some(3), None),
        (None, Some(0)),
        (None, Some(6)),
    ] {
        assert!(
            core.execute(Command::Decision { event, candidate })
                .is_err()
        );
    }
}

#[test]
fn decision_v2_legacy_report_never_invents_a_certificate() {
    let core = SessionCore::new(SessionInit::hotseat());
    let mut value: serde_json::Value =
        serde_json::from_str(&core.export_record_json().unwrap()).unwrap();
    value["version"] = 2.into();
    value["history"]["nodes"][0]["resignation"] =
        serde_json::json!({"reason":"verified_opponent_vcf", "by":"Black", "candidates":5});
    let mut loaded = SessionCore::import_record_json(&value.to_string()).unwrap();
    let output = crate::repl::process_line("decision", &mut loaded, false)
        .lines
        .join("\n");
    assert!(
        output
            .contains("Original decision log/proofs unavailable (legacy record); cannot preview.")
    );
    assert!(!output.contains("Verified original VCF"));
    assert_eq!(
        loaded.execute(Command::Decision {
            event: None,
            candidate: Some(1)
        }),
        Err(crate::command::CommandError::DecisionUnavailable)
    );
}

#[test]
fn decision_direct_take_invalidates_same_board_resume_and_preserves_audit() {
    let mut core = concession();
    let board = core.board();
    let original = core.resignation_events()[0].clone();
    let revision = core.analysis_revision();
    let events = core.execute_take(SeatSel::Black).unwrap();
    assert_ne!(core.analysis_revision(), revision);
    assert_eq!(core.board(), board);
    assert_eq!(core.active_resignation(), None);
    assert!(events.contains(&Event::Resumed { by: Color::Black }));
    let mut resumed = original;
    resumed.resumed = true;
    assert_eq!(core.resignation_events(), &[resumed]);
    assert!(
        report(&core, None, None)
            .unwrap()
            .join("\n")
            .contains("historical/resumed")
    );
}

#[test]
fn decision_direct_mutators_invalidate_analysis_context() {
    let mv = Move::new(0, 0).unwrap();
    let cases = [
        (Command::New, vec![], true),
        (
            Command::SetSims {
                seat: SeatSel::Black,
                n: 4,
            },
            vec![],
            true,
        ),
        (
            Command::Release {
                seat: SeatSel::Black,
            },
            vec![],
            true,
        ),
        (
            Command::Take {
                seat: SeatSel::Both,
            },
            vec![],
            true,
        ),
        (Command::Play { mv }, vec![], false),
        (Command::Back { n: 1 }, vec![Command::Play { mv }], false),
        (
            Command::Forward { n: 1 },
            vec![Command::Play { mv }, Command::Back { n: 1 }],
            false,
        ),
        (
            Command::Var { index: 0 },
            vec![Command::Play { mv }, Command::Back { n: 1 }],
            false,
        ),
    ];
    let mut unchanged = Vec::new();
    for (command, setup, same_board) in cases {
        let init = SessionInit {
            seed: Some(42),
            ..SessionInit::hotseat()
        };
        let mut direct = SessionCore::new(init.clone());
        let mut dispatched = SessionCore::new(init);
        for setup_command in setup {
            direct.execute(setup_command.clone()).unwrap();
            dispatched.execute(setup_command).unwrap();
        }
        let board = direct.board();
        let revision = direct.analysis_revision();
        let result = match command {
            Command::New => direct.execute_new(),
            Command::SetSims { seat, n } => direct.execute_set_sims(seat, n),
            Command::Release { seat } => direct.execute_release(seat),
            Command::Take { seat } => direct.execute_take(seat),
            Command::Play { mv } => direct.execute_play(mv),
            Command::Back { n } => direct.execute_back(n),
            Command::Forward { n } => direct.execute_forward(n),
            Command::Var { index } => direct.execute_var(index),
            _ => unreachable!(),
        };
        assert_eq!(result, dispatched.execute(command.clone()), "{command:?}");
        assert!(result.is_ok(), "{command:?}");
        assert_eq!(direct.board() == board, same_board, "{command:?}");
        assert_eq!(direct.snapshot(), dispatched.snapshot(), "{command:?}");
        assert_eq!(
            direct.export_record_json().unwrap(),
            dispatched.export_record_json().unwrap(),
            "{command:?}"
        );
        if direct.analysis_revision() == revision {
            unchanged.push(command);
        }
    }
    assert!(
        unchanged.is_empty(),
        "direct mutations retained revision: {unchanged:?}"
    );
}

#[test]
fn decision_direct_forward_invalidates_before_partial_progress_error() {
    let mut core = SessionCore::new(SessionInit::hotseat());
    core.execute_play(Move::new(0, 0).unwrap()).unwrap();
    let child = core.board();
    core.execute_back(1).unwrap();
    let root = core.board();
    let revision = core.analysis_revision();
    assert_eq!(
        core.execute_forward(2),
        Err(crate::command::CommandError::ForwardAtLeaf)
    );
    assert_eq!(core.board(), child);
    assert_ne!(core.analysis_revision(), revision);
    core.execute_back(1).unwrap();
    assert_eq!(core.board(), root);
    assert_ne!(
        core.analysis_revision(),
        revision,
        "equal-board roundtrip is not equal context"
    );
}

#[test]
fn decision_dispatch_only_mutators_invalidate_on_success_and_error() {
    let mut core = SessionCore::new(SessionInit::hotseat());
    for command in [
        Command::Step,
        Command::Done,
        Command::More,
        Command::ChooseColor {
            color: Color::Black,
        },
        Command::NextPuzzle,
        Command::PrevPuzzle,
    ] {
        let revision = core.analysis_revision();
        assert!(core.execute(command.clone()).is_err(), "{command:?}");
        assert_ne!(core.analysis_revision(), revision, "{command:?}");
    }
    let mut core = SessionCore::new(SessionInit::hotseat().with_swap2(true));
    for command in [
        Command::Play {
            mv: Move::new(0, 0).unwrap(),
        },
        Command::Back { n: 1 },
        Command::Play {
            mv: Move::new(0, 0).unwrap(),
        },
        Command::Play {
            mv: Move::new(0, 1).unwrap(),
        },
        Command::Play {
            mv: Move::new(0, 2).unwrap(),
        },
        Command::Done,
        Command::More,
        Command::Play {
            mv: Move::new(0, 3).unwrap(),
        },
        Command::Play {
            mv: Move::new(0, 4).unwrap(),
        },
        Command::Done,
        Command::ChooseColor {
            color: Color::Black,
        },
    ] {
        let revision = core.analysis_revision();
        core.execute(command.clone()).unwrap();
        assert_ne!(core.analysis_revision(), revision, "{command:?}");
    }
    let pack = crate::puzzle::PuzzlePack::from_json(
        r#"{"v":1,"count":2,"items":[{"i":0,"a":"black","b":[],"w":[],"l":[]},{"i":1,"a":"black","b":[],"w":[],"l":[]}]}"#,
    ).unwrap();
    let init = SessionInit::puzzle("identity-test-pack.json".into(), pack, 0).unwrap();
    let mut core = SessionCore::new(init);
    for command in [Command::NextPuzzle, Command::PrevPuzzle] {
        let revision = core.analysis_revision();
        let board = core.board();
        core.execute(command.clone()).unwrap();
        assert_eq!(core.board(), board);
        assert_ne!(core.analysis_revision(), revision, "{command:?}");
    }
    let mut core = SessionCore::new(SessionInit {
        seed: Some(42),
        ..SessionInit::versus_ai(
            Color::White,
            AiConfig {
                simulations: 3,
                evaluator: EvaluatorKind::Tactics,
                ..AiConfig::default()
            },
        )
    });
    let revision = core.analysis_revision();
    core.execute(Command::Step).unwrap();
    assert_ne!(core.analysis_revision(), revision);
}

#[test]
fn decision_small_pages_include_every_long_line() {
    let core = concession();
    let lines = report(&core, None, None).unwrap();
    let pages = crate::decision::pages(&lines, 23, 7);
    assert!(pages.len() > 5);
    let reconstructed: String = pages
        .iter()
        .flat_map(|p| {
            assert!(p.len() <= 7);
            assert!(p.iter().all(|s| s.chars().count() <= 23));
            p[1..p.len() - 1].iter().cloned()
        })
        .collect();
    assert_eq!(reconstructed, lines.concat());
}
