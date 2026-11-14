use engine::{Color, Move, Status};
use player::PlayerOptions;
use rand::prelude::StdRng;
use selfplay::game::{Outcome, WinReason, play_game, play_game_with_opening};
use selfplay::opening::{Opening, OpeningAnswer, OpeningProvenance, OpeningSource};
use std::time::Instant;

fn mv(r: u8, c: u8) -> Move {
    Move::new(r, c).expect("test coordinates on board")
}

fn options(sims: u32) -> PlayerOptions {
    PlayerOptions {
        simulations: sims,
        ..PlayerOptions::default()
    }
}

struct FixedOpening(Opening);

impl OpeningSource for FixedOpening {
    fn opening(&mut self, _rng: &mut StdRng) -> Opening {
        self.0.clone()
    }
}

#[test]
fn injectable_opening_executes_the_add_two_swap2_branch() {
    let opening = Opening::new(
        [mv(5, 8), mv(5, 10), mv(7, 9)],
        OpeningAnswer::AddTwo {
            stones: [mv(9, 8), mv(9, 10)],
            final_choice: Color::White,
        },
        OpeningProvenance::Listed {
            list: "fixed-test".to_owned(),
            index: 11,
        },
    );
    let board = opening.board().expect("five stone ceremony completed.");

    assert_eq!(board.empty_moves().count(), 220);
    assert_eq!(board.to_move(), Color::White);

    let game = play_game_with_opening(
        11,
        options(1),
        options(1),
        &mut FixedOpening(opening.clone()),
    )
    .expect("Game from fixed opening completes");

    assert_eq!(game.opening, opening);
    assert_eq!(game.plies.len(), 10);
}

#[test]
fn same_seed_and_seat_options_reproduce_entire_game() {
    let first = play_game(11, options(1), options(1)).expect("first game completes");
    let second = play_game(11, options(1), options(1)).expect("first game completes");

    assert_eq!(first, second);
    assert_eq!(
        first.moves().collect::<Vec<_>>(),
        second.moves().collect::<Vec<_>>()
    );
    assert_eq!(first.outcome, second.outcome);
}

#[test]
fn recorded_moves_replay_legally_to_the_recorded_outcome() {
    let game = play_game(11, options(1), options(1)).expect("Game completes");
    let mut board = game.opening.board().expect("opening is legal");
    for (ply, record) in game.plies.iter().enumerate() {
        assert!(board.is_legal(record.mv), "Illegal move at ply {ply}");
        board.play(record.mv).expect("recorded move replays");
    }
    let replayed = match board.status() {
        Status::Won(winner) => Outcome::Won {
            winner,
            reason: WinReason::FiveOrMore,
        },
        Status::Draw => Outcome::Draw,
        Status::Ongoing => panic!("recorded game ended prematurely"),
    };
    assert_eq!(replayed, game.outcome);
}

#[test]
// MANUAL: this uses a budget-2 Black seat and plays a full game. Run explicitly
// when validating per-seat visit sums; default tests stay at budget 1.
#[ignore]
fn tiny_budget_game_records_raw_visits_for_every_searched_ply() {
    let game = play_game(11, options(2), options(1)).expect("tiny-budget game completes");
    let mut searched_plies = 0;

    for ply in &game.plies {
        let Some(search) = &ply.search else {
            continue;
        };
        searched_plies += 1;
        let simulations = match search.side_to_move {
            Color::Black => 2,
            Color::White => 1,
        };
        assert!(!search.visits.is_empty());
        assert!(search.visits.iter().all(|(_, count)| *count > 0));
        assert_eq!(
            search.visits.iter().map(|(_, count)| count).sum::<u32>(),
            simulations,
            "root child visits must sum to the recording seat's simulations"
        );
    }
    assert!(searched_plies > 0);
}

#[test]
// MANUAL: this budget-8 full game can run for minutes in debug. The budget-1
// tests above cover the same runner invariants without the slow search.
#[ignore]
fn tiny_budget_game_completes_and_records_search_metadata() {
    let started = Instant::now();
    let game = play_game(11, options(8), options(8)).expect("the smoke game completes");

    assert!(!game.plies.is_empty());
    assert!(game.plies.len() <= 222);
    assert_eq!(game.moves().count(), game.plies.len());
    assert!(game.plies.iter().any(|ply| ply.search.is_some()));
    assert!(game.plies.iter().all(|ply| {
        ply.search
            .as_ref()
            .is_none_or(|search| search.root_value.is_finite())
    }));
    assert!(!game.black_options.resign);
    assert!(!game.white_options.resign);

    eprintln!(
        "smoke game: {} plies, {:?}, {:.3?}",
        game.plies.len(),
        game.outcome,
        started.elapsed()
    );
}
