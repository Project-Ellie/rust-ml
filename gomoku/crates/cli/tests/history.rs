use cli::history::{ForwardOutcome, History, HistoryError, PlayOutcome};
use engine::{Board, Color, PlayError, Status};
use proptest::prelude::*;

mod common;
use common::mv;

fn empty_board() -> Board {
    Board::new()
}

fn opening_board() -> Board {
    Board::from_position(&[mv(7, 7), mv(6, 8)], &[mv(6, 7)], Color::White).unwrap()
}

fn replay_cursor_path(history: &History) -> Board {
    let mut board = empty_board();
    for &m in history.board().moves() {
        board.play(m).unwrap();
    }
    board
}

#[test]
fn new_history_has_empty_root() {
    let history = History::new();
    assert_eq!(history.ply(), 0);
    assert_eq!(history.board(), empty_board());
    assert!(history.children(history.cursor()).is_empty());
}

#[test]
fn from_opening_uses_given_root() {
    let opening = opening_board();
    let history = History::from_opening(&opening);
    assert_eq!(history.board(), opening);
    assert_eq!(history.board().to_move(), Color::White);
    assert_eq!(history.ply(), 0);
}

#[test]
fn play_appends_and_follows_existing() {
    let mut history = History::new();
    let m77 = mv(7, 1);
    let m67 = mv(6, 7);

    let out = history.play(m77).unwrap();
    assert!(matches!(out, PlayOutcome::Moved(_)));
    assert_eq!(history.ply(), 1);
    assert_eq!(history.board().to_move(), Color::White);

    history.back(1);
    let out = history.play(m77).unwrap();
    assert!(matches!(out, PlayOutcome::FollowedExisting(_)));
    assert_eq!(history.ply(), 1);

    history.back(1);
    let out = history.play(m67).unwrap();
    assert!(matches!(out, PlayOutcome::Moved(_)));
    assert_eq!(history.ply(), 1);
}

#[test]
fn navigation_branch_walk() {
    let mut history = History::new();
    let main_line = [mv(7, 7), mv(6, 7), mv(7, 8), mv(6, 8), mv(7, 9), mv(6, 9)];
    for &m in &main_line {
        history.play(m).unwrap();
    }
    assert_eq!(history.ply(), 6);

    let backed = history.back(3);
    assert_eq!(backed, 3);
    assert_eq!(history.ply(), 3);

    let branch = mv(8, 7);
    let out = history.play(branch).unwrap();
    assert!(matches!(out, PlayOutcome::Moved(_)));
    assert_eq!(history.ply(), 4);

    let backed2 = history.back(1);
    assert_eq!(backed2, 1);
    assert_eq!(history.ply(), 3);

    let out = history.play(main_line[3]).unwrap();
    assert!(matches!(out, PlayOutcome::FollowedExisting(_)));
    assert_eq!(history.ply(), 4);

    history.back(1);
    let forward = history.forward(1);
    let choices = match forward {
        ForwardOutcome::NeedsChoice(opts) => opts,
        other => panic!("expected NeedsChoice, got {other:?}"),
    };
    assert_eq!(choices, vec![main_line[3], branch]);

    let chosen = history.var(1).unwrap();
    assert_eq!(chosen, branch);
    assert_eq!(history.ply(), 4);

    let extension = mv(8, 8);
    let out = history.play(extension).unwrap();
    assert!(matches!(out, PlayOutcome::Moved(_)));
    assert_eq!(history.ply(), 5);

    let leaf = history.forward(1);
    assert!(matches!(leaf, ForwardOutcome::AtLeaf));

    let backed3 = history.back(100);
    assert_eq!(backed3, 5);
    assert_eq!(history.ply(), 0);
}

#[test]
fn illegal_move_returns_error() {
    let mut history = History::new();
    history.play(mv(7, 7)).unwrap();

    let err = history.play(mv(7, 7)).unwrap_err();
    assert_eq!(err, HistoryError::IllegalMove(PlayError::Occupied));
}

#[test]
fn playing_after_game_over_returns_error() {
    let mut history = History::new();
    let win_moves = [
        mv(5, 5),
        mv(0, 1),
        mv(5, 6),
        mv(0, 2),
        mv(5, 7),
        mv(0, 3),
        mv(5, 8),
        mv(0, 4),
        mv(5, 9),
    ];
    for &m in &win_moves {
        history.play(m).unwrap();
    }
    assert_eq!(history.board().status(), Status::Won(Color::Black));
    let err = history.play(mv(0, 5)).unwrap_err();
    assert_eq!(err, HistoryError::IllegalMove(PlayError::GameOver));
}

#[test]
fn var_out_of_bounds_returns_an_error() {
    let mut history = History::new();
    history.play(mv(6, 6)).unwrap();
    history.play(mv(6, 7)).unwrap();
    history.back(1);
    let err = history.var(5).unwrap_err();
    assert!(matches!(err, HistoryError::VarOutOfBounds { .. }));
}

#[test]
fn from_opening_play_continues_correct_side() {
    let opening = opening_board();
    let mut history = History::from_opening(&opening);
    let next = mv(5, 7);
    history.play(next).unwrap();
    assert_eq!(history.ply(), 1);
    assert_eq!(history.board().to_move(), Color::Black);
    assert!(history.board().stone_at(next).is_some());
}

/// A hundred random walks
#[derive(Debug, Clone)]
enum HistCmd {
    Play(usize),
    Back(u32),
    Forward(u32),
    Var(usize),
}

fn hist_cmd() -> impl Strategy<Value = HistCmd> {
    prop_oneof![
        any::<usize>().prop_map(HistCmd::Play),
        any::<u32>().prop_map(HistCmd::Back),
        any::<u32>().prop_map(HistCmd::Forward),
        any::<usize>().prop_map(HistCmd::Var),
    ]
}

proptest! {
    #[test]
    fn random_walks_keep_board_consistent(cmds in prop::collection::vec(hist_cmd(), 1..100)) {
        let mut history = History::new();

        for cmd in cmds {
            match cmd {
                HistCmd::Play(idx) => {
                    let board = history.board();
                    if board.status() != Status::Ongoing { continue;}
                    let legals: Vec<_> = board.empty_moves().collect();
                    if legals.is_empty() { continue; }
                    let mv = legals[idx % legals.len()];
                    let _ = history.play(mv);
                }

                HistCmd::Back(n) => {
                    history.back(n);
                }
                HistCmd::Forward(n) => {
                    if let ForwardOutcome::NeedsChoice(children) = history.forward(n)
                    && !children.is_empty() {
                        let _ = history.var(0);
                    }
                }
                HistCmd::Var(idx) => {
                    let children = history.children(history.cursor());
                    if !children.is_empty() {
                        let _ = history.var(idx % children.len());
                    }
                }
            }
            let replayed = replay_cursor_path(&history);
            assert_eq!(history.board(), replayed);

            assert_eq!(history.ply(), history.board().moves().len() as u32);
        }
    }
}
