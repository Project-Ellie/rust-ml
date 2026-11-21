//! The arena: pairing structure, tallying, and the Wilson verdict.

use engine::{Color, Move};
use player::PlayerOptions;
use rand::prelude::StdRng;
use selfplay::arena::{ArenaConfig, ArenaError, GameResult, play_arena, wilson_interval};
use selfplay::opening::{Opening, OpeningAnswer, OpeningProvenance, OpeningSource};

struct FixedOpening(Opening);

impl OpeningSource for FixedOpening {
    fn opening(&mut self, _rng: &mut StdRng) -> Opening {
        self.0.clone()
    }
}

fn mv(row: u8, col: u8) -> Move {
    Move::new(row, col).expect("test coordinates on board")
}

fn fixed_opening() -> FixedOpening {
    FixedOpening(Opening::new(
        [mv(5, 5), mv(10, 10), mv(5, 6)],
        OpeningAnswer::Take(Color::Black),
        OpeningProvenance::Listed {
            list: "test".to_owned(),
            index: 0,
        },
    ))
}

fn baseline_factory() -> Box<dyn mcts::Evaluator> {
    Box::new(baseline::BaselineEvaluator::new(baseline::Device::flex()))
}

#[test]
fn wilson_interval_matches_reference_values() {
    // Reference: p = 0.5, n = 10 → (0.237, 0.763) at 95%.
    let (lower, upper) = wilson_interval(5.0, 10);
    assert!((lower - 0.2366).abs() < 1e-3, "lower {lower}");
    assert!((upper - 0.7634).abs() < 1e-3, "upper {upper}");

    // A perfect score still has an honest lower bound, not 1.0.
    let (lower, upper) = wilson_interval(10.0, 10);
    assert!(lower < 0.8 && lower > 0.6, "lower {lower}");
    assert_eq!(upper, 1.0);

    // No games: the full range, no verdict.
    assert_eq!(wilson_interval(0.0, 0), (0.0, 1.0));
}

#[test]
fn a_pair_swaps_colors_on_one_seed_and_tallies() {
    let config = ArenaConfig {
        games: 2,
        sims: 1,
        seed_base: 11,
    };
    let result = play_arena(
        config,
        PlayerOptions::default(),
        &mut fixed_opening(),
        baseline_factory,
        baseline_factory,
    )
    .expect("the arena plays out");

    assert_eq!(result.game_count(), 2);
    assert_eq!(result.games[0].seed, 11);
    assert_eq!(result.games[1].seed, 11, "a pair shares its seed");
    assert_eq!(result.games[0].candidate_color, Color::Black);
    assert_eq!(result.games[1].candidate_color, Color::White);
    assert_eq!(result.played.len(), 2, "the played games are returned");
    assert_eq!(result.played[0].seed, 11);

    let (wins, draws, losses) = result.tally();
    assert_eq!(wins + draws + losses, 2);
    let expected_score: f64 = result.games.iter().map(|game| game.result.score()).sum();
    assert_eq!(result.score, expected_score);
    assert!(result.interval.0 <= result.interval.1);
}

#[test]
fn an_odd_or_zero_game_count_is_rejected() {
    for games in [0, 3] {
        let error = play_arena(
            ArenaConfig {
                games,
                sims: 1,
                seed_base: 0,
            },
            PlayerOptions::default(),
            &mut fixed_opening(),
            baseline_factory,
            baseline_factory,
        )
        .expect_err("odd and zero counts are rejected");
        assert!(matches!(error, ArenaError::Config(_)));
    }
}

#[test]
fn game_result_scores() {
    assert_eq!(GameResult::Win.score(), 1.0);
    assert_eq!(GameResult::Draw.score(), 0.5);
    assert_eq!(GameResult::Loss.score(), 0.0);
}
