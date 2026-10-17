> **Opt-in solution — read only after you have tried the chapter, or if you have been stuck for more than twenty minutes.**
>
> This file quotes the two files that changed for chapter 07:
> `gomoku/crates/cli/tests/acceptance.rs` and
> `gomoku/crates/cli/src/lib.rs`. The acceptance test file was
> introduced at commit `d4079c8`, amended at commit `febcb0f` to use
> a uniform-random baseline, and cleaned up at commit `d0570fd` (an
> unused import the clippy gate caught); the text below is the final
> `d0570fd` state. The quoted text was verified byte-for-byte against
> the reference worktree with `git show d0570fd:<path>`.

# Chapter 07 — opt-in solution

## `gomoku/crates/cli/tests/acceptance.rs`

```rust
//! Task-7 acceptance tests: proof, not vibes.
//!
//! * Golden transcript: a seeded scripted study session reproduces
//!   event-for-event and ends in the expected snapshot.
//! * Scripted game: a human forcing line beats the tactics AI.
//! * Measured strength: tactics AI at 50/200/800 sims vs a near-random
//!   baseline (1 sim, temperature 1.0), 20 seeded games each.

use cli::actor::{AiConfig, EvaluatorKind, Occupant, OccupantKind};
use cli::command::{Command, Event, SeatSel};
use cli::session::{SessionCore, SessionInit, Snapshot};
use engine::{Color, Move, Status};
use rand::{RngExt, SeedableRng};

fn mv(r: u8, c: u8) -> Move {
    Move::new(r, c).unwrap()
}

fn play(core: &mut SessionCore, r: u8, c: u8) -> Vec<Event> {
    core.execute(Command::Play(mv(r, c))).unwrap()
}

fn step(core: &mut SessionCore) -> Vec<Event> {
    core.execute(Command::Step).unwrap()
}

fn default_ai_config() -> AiConfig {
    AiConfig {
        evaluator: EvaluatorKind::Tactics,
        simulations: 200,
        c_puct: 1.5,
        temperature: 0.0,
        noise_epsilon: 0.0,
        explicit_selection: false,
    }
}

#[test]
fn golden_transcript_seeded_study_session() {
    let mut core = SessionCore::new(SessionInit::ai_vs_ai(default_ai_config(), 42));

    let mut events = Vec::new();

    // 6 AI-vs-AI opening steps (schedule active for the first 10 plies).
    for _ in 0..6 {
        events.extend(step(&mut core));
    }

    // Per-seat sims mid-session.
    events.extend(
        core.execute(Command::SetSims {
            seat: SeatSel::Black,
            n: 100,
        })
        .unwrap(),
    );
    events.extend(
        core.execute(Command::SetSims {
            seat: SeatSel::White,
            n: 50,
        })
        .unwrap(),
    );

    // Steal Black's seat.
    events.extend(
        core.execute(Command::Take {
            seat: SeatSel::Black,
        })
        .unwrap(),
    );

    // Walk back two plies and play a human branch move.
    events.extend(core.execute(Command::Back { n: 2 }).unwrap());
    events.extend(play(&mut core, 0, 0));

    // Return to the branch point and take the original main-line child.
    events.extend(core.execute(Command::Back { n: 1 }).unwrap());
    events.extend(core.execute(Command::Var { index: 0 }).unwrap());

    // Hand Black back to the AI and continue two more plies.
    events.extend(
        core.execute(Command::Release {
            seat: SeatSel::Black,
        })
        .unwrap(),
    );
    events.extend(step(&mut core));
    events.extend(step(&mut core));

    let expected = vec![
        Event::Moved {
            mv: mv(4, 3),
            by: OccupantKind::Ai,
        },
        Event::Moved {
            mv: mv(9, 6),
            by: OccupantKind::Ai,
        },
        Event::Moved {
            mv: mv(3, 1),
            by: OccupantKind::Ai,
        },
        Event::Moved {
            mv: mv(11, 12),
            by: OccupantKind::Ai,
        },
        Event::Moved {
            mv: mv(13, 3),
            by: OccupantKind::Ai,
        },
        Event::Moved {
            mv: mv(7, 3),
            by: OccupantKind::Ai,
        },
        Event::ConfigChanged {
            seat: Color::Black,
            sims: 100,
        },
        Event::ConfigChanged {
            seat: Color::White,
            sims: 50,
        },
        Event::SeatChanged {
            seat: Color::Black,
            now: OccupantKind::Human,
        },
        Event::CursorMoved { ply: 4 },
        Event::Moved {
            mv: mv(0, 0),
            by: OccupantKind::Human,
        },
        Event::Branched {
            options: vec![mv(13, 3), mv(0, 0)],
        },
        Event::CursorMoved { ply: 4 },
        Event::CursorMoved { ply: 5 },
        Event::SeatChanged {
            seat: Color::Black,
            now: OccupantKind::Ai,
        },
        Event::Moved {
            mv: mv(3, 0),
            by: OccupantKind::Ai,
        },
        Event::Branched {
            options: vec![mv(7, 3), mv(3, 0)],
        },
        Event::Moved {
            mv: mv(2, 9),
            by: OccupantKind::Ai,
        },
    ];

    assert_eq!(
        events, expected,
        "seeded study-session transcript must reproduce exactly"
    );

    let expected_snapshot = Snapshot {
        ply: 7,
        branch_options: vec![],
        black: OccupantKind::Ai,
        white: OccupantKind::Ai,
        status: Status::Ongoing,
        last_move: Some(mv(2, 9)),
    };
    assert_eq!(
        core.snapshot(),
        expected_snapshot,
        "final snapshot must match the transcript"
    );
}

#[test]
fn scripted_human_beats_tactics_ai() {
    // Classic fork: Black plays (7,7) to create two open fours. White
    // can only block one; Black wins with the other. The scattered O
    // stones are quiet filler taken from the engine tactics tests.
    let opening = engine::reference::board_from_ascii(
        "
        . . . . . . . . . . . . . . O
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . O . . . . . . .
        . . . . . . . X . . . . . . .
        . . . . . . . X . . . . . . .
        . . . . . . . X . . . . . . .
        . . . O X X X . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . O . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        O . . . . . . . . . . . . . O
        ",
    );
    assert_eq!(opening.to_move(), Color::Black);

    let ai_cfg = AiConfig {
        simulations: 200,
        ..default_ai_config()
    };
    let mut core = SessionCore::new(SessionInit {
        black: Occupant::Human,
        white: Occupant::Ai(ai_cfg.clone()),
        default_config: ai_cfg,
        black_sims: None,
        white_sims: None,
        seed: Some(7),
        opening,
    });

    // Human plays the fork move.
    let events = play(&mut core, 7, 7);
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::Moved {
                by: OccupantKind::Human,
                ..
            }
        )),
        "human move must be emitted"
    );
    assert_eq!(core.board().status(), Status::Ongoing);

    // AI White answers. The 200-sim tactics AI may or may not find a
    // block; the fork is a forcing win regardless.
    let events = step(&mut core);
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::Moved {
                by: OccupantKind::Ai,
                ..
            }
        )),
        "AI must respond"
    );
    assert_eq!(core.board().status(), Status::Ongoing);

    // Human completes one of the open fours and wins.
    let forced_wins = engine::immediate_wins(&core.board(), Color::Black);
    assert!(
        !forced_wins.is_empty(),
        "after the AI response Black must have at least one winning move"
    );
    let winning_mv = forced_wins.iter().next().unwrap();
    let events = play(&mut core, winning_mv.row(), winning_mv.col());
    assert!(
        events.iter().any(|e| matches!(e, Event::GameOver { .. })),
        "final human move must end the game"
    );
    assert_eq!(core.board().status(), Status::Won(Color::Black));
    assert_eq!(
        core.board().moves().last().copied(),
        Some(winning_mv),
        "the final move must belong to the scripted side"
    );
}

/// Run explicitly: cargo test -p cli --test acceptance -- --ignored --nocapture
///
/// Measures the tactics AI's win rate against a uniform-random baseline
/// at 50/200/800 simulations. Black is the AI at the given sims; White is
/// the test driver playing seeded uniform-random legal moves via `Play`
/// (White's seat is Human, so the study table accepts them). Games are
/// seeded so the measurement is reproducible. Output is intended to be
/// quoted in the tutorial chapter.
#[ignore]
#[test]
fn sims_vs_strength_measured() {
    fn run_game(black_sims: u32, seed: u64) -> Status {
        let black_cfg = AiConfig {
            evaluator: EvaluatorKind::Tactics,
            simulations: black_sims,
            c_puct: 1.5,
            temperature: 0.0,
            noise_epsilon: 0.0,
            explicit_selection: true,
        };
        let mut init = SessionInit::ai_vs_ai(black_cfg.clone(), seed);
        init.white = Occupant::Human;
        let mut core = SessionCore::new(init);
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed ^ 0x5eed);
        while core.board().status() == Status::Ongoing {
            if core.board().to_move() == Color::Black {
                core.execute(Command::Step).unwrap();
            } else {
                let legal: Vec<Move> = core.board().empty_moves().collect();
                let mv = legal[rng.random_range(0..legal.len())];
                core.execute(Command::Play(mv)).unwrap();
            }
        }
        core.board().status()
    }

    const GAMES_PER_LEVEL: usize = 20;
    const BASE_SEED: u64 = 2026;
    let levels = [50, 200, 800];

    println!("sims-vs-strength (tactics AI Black vs uniform-random White)");
    println!("games per level: {GAMES_PER_LEVEL}, base seed: {BASE_SEED}");
    println!(
        "{:<6} {:>6} {:>6} {:>6} {:>6} {:>8}",
        "sims", "wins", "losses", "draws", "games", "win_rate"
    );

    for sims in levels {
        let mut black_wins = 0;
        let mut white_wins = 0;
        let mut draws = 0;
        for g in 0..GAMES_PER_LEVEL {
            let seed = BASE_SEED + g as u64 + sims as u64 * 10_000;
            match run_game(sims, seed) {
                Status::Won(Color::Black) => black_wins += 1,
                Status::Won(Color::White) => white_wins += 1,
                Status::Draw => draws += 1,
                Status::Ongoing => panic!("run_game only returns terminal states"),
            }
        }
        let rate = black_wins as f64 / GAMES_PER_LEVEL as f64;
        println!(
            "{:<6} {:>6} {:>6} {:>6} {:>6} {:>7.0}%",
            sims,
            black_wins,
            white_wins,
            draws,
            GAMES_PER_LEVEL,
            rate * 100.0
        );
    }
}
```

## `gomoku/crates/cli/src/lib.rs`

```rust
//! Terminal Gomoku client library.
//!
//! The `cli` crate is split into a headless library (history, command,
//! session, actor) and a thin binary front-end. The `tui` module holds
//! the existing terminal UI pieces moved here unchanged.

#![deny(missing_docs)]

pub mod actor;
pub mod command;
pub mod fmt;
pub mod history;
pub mod session;
pub mod tui;
```
