use engine::{Color, Move};
use player::PlayerOptions;
use selfplay::game::{Game, Outcome};
use selfplay::generate::generate_games_with;
use selfplay::opening::{Opening, OpeningAnswer, OpeningProvenance};
use selfplay::schedule::ExplorationSchedule;

fn mv(row: u8, col: u8) -> Move {
    Move::new(row, col).expect("test coordinates on board")
}

fn fabricated_game(seed: u64) -> Result<Game, selfplay::game::GameError> {
    let row = (seed % 15) as u8;
    let opening = Opening::new(
        [mv(row, 5), mv(row, 6), mv(row, 7)],
        OpeningAnswer::Take(Color::Black),
        OpeningProvenance::Generated,
    );

    Ok(Game {
        seed,
        schedule: ExplorationSchedule::default(),
        black_options: PlayerOptions::default(),
        white_options: PlayerOptions::default(),
        opening,
        plies: Vec::new(),
        outcome: Outcome::Draw,
        certified_suffix: None,
    })
}

#[test]
fn forced_collision_is_rejected_and_rerolled_with_seed_plus_attempt() {
    let games = generate_games_with([7, 7], fabricated_game).expect("generation completes");

    assert_eq!(games.len(), 2);
    assert_eq!(games[0].seed, 7);
    assert_eq!(
        games[1].seed, 8,
        "derived seed must be base seed 7 + attempt 1"
    );
    assert_ne!(games[0].opening.initial, games[1].opening.initial);
}
