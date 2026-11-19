use player::{GuardConfig, OwnTssConfig, PlayerOptions};
use selfplay::game::play_game;
use selfplay::record::GameRecord;
use selfplay::schedule::{EXPLORATION_PLIES, ExplorationSchedule};

fn base_options(sims: u32) -> PlayerOptions {
    PlayerOptions {
        simulations: sims,
        ..PlayerOptions::default()
    }
}

#[test]
fn first_ten_plies_explore_then_selection_is_deterministic_without_changing_budgets() {
    let base = PlayerOptions {
        simulations: 777,
        c_puct: 1.75,
        temperature: 4.0,
        noise_epsilon: 0.8,
        noise_alpha: 0.9,
        own: OwnTssConfig::baseline(),
        guard: GuardConfig::baseline(),
        resign: true,
    };

    let schedule = ExplorationSchedule::default();
    assert_eq!(EXPLORATION_PLIES, 10);

    for ply in [0, 1, EXPLORATION_PLIES - 1] {
        let effective = schedule.options_for_ply(base, ply);
        assert_eq!(effective.simulations, 777);
        assert_eq!(effective.c_puct, 1.75);
        assert_eq!(effective.own, OwnTssConfig::baseline());
        assert_eq!(effective.guard, GuardConfig::baseline());
        assert!(effective.resign);
        assert_eq!(effective.temperature, 1.0);
        assert_eq!(effective.noise_epsilon, 0.25);
        assert_eq!(effective.noise_alpha, 0.3);
    }

    for ply in [EXPLORATION_PLIES, EXPLORATION_PLIES + 1, 100] {
        let effective = schedule.options_for_ply(base, ply);
        assert_eq!(effective.simulations, 777);
        assert_eq!(effective.c_puct, 1.75);
        assert_eq!(effective.own, OwnTssConfig::baseline());
        assert_eq!(effective.guard, GuardConfig::baseline());
        assert!(effective.resign);
        assert_eq!(effective.temperature, 0.0);
        assert_eq!(effective.noise_epsilon, 0.0);
    }
}

#[test]
// MANUAL: two full games are unnecessary for the isolated schedule check and
// can run for minutes in debug; the binary's budget-1 fixture checks recording.
#[ignore]
fn scheduled_game_is_reproducible_and_records_its_schedule_and_pairing() {
    let black = base_options(2);
    let white = base_options(1);

    let first = play_game(29, black, white).expect("first scheduled game completes");
    let second = play_game(29, black, white).expect("second scheduled game completes");

    let first_record = GameRecord::from_game(&first);
    let second_record = GameRecord::from_game(&second);

    assert_eq!(first_record, second_record);
    assert_eq!(
        first_record.header.config.exploration,
        ExplorationSchedule::default()
    );
    assert_eq!(first_record.header.config.black.simulations, 2);
    assert_eq!(first_record.header.config.white.simulations, 1);
}
