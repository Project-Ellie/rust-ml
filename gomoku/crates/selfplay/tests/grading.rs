use engine::Color;
use player::PlayerOptions;
use selfplay::game::{Outcome, play_game};

fn options(simulations: u32) -> PlayerOptions {
    PlayerOptions {
        simulations,
        ..PlayerOptions::default()
    }
}

#[test]
// MANUAL: four full games — minutes in debug; run explicitly with the grading
// target selected and --ignored. (Tutorial convention: use --release.)
//
// The seeds are measured fixed seeds — four short games, all won by
// baseline-16 in the reference — and pinning avoids turning a cheap sanity
// check into a flaky Elo estimate.
#[ignore]
fn higher_budget_seat_wins_more_often_on_measured_seeds() {
    let seeds = [11, 1, 7, 18];
    let black = options(16);
    let white = options(1);

    let mut stronger_wins = 0usize;
    let mut weaker_wins = 0usize;

    for seed in seeds {
        let game = play_game(seed, black, white).expect("grading game completes");
        let ply_count = game.plies.len();
        match game.outcome {
            Outcome::Won { winner, .. } => {
                if winner == Color::Black {
                    stronger_wins += 1;
                } else {
                    weaker_wins += 1;
                }
            }
            Outcome::Draw => {}
        }
        eprintln!("seed {seed}: {ply_count} plies, outcome {:?}", game.outcome);
    }

    assert!(
        stronger_wins > weaker_wins,
        "stronger seat must win strictly more often (got {stronger_wins} vs {weaker_wins})"
    );
}
