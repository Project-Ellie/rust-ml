//! Labelling of boards for training purposes

use engine::{Board, Move, MoveSet, forced_blocks, immediate_wins};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TacticalClass {
    Win,
    Block,
    Quiet,
}

/// side to move can win: Win, else if opponent could: Block,
/// else Quiet
pub fn classify(board: &Board) -> TacticalClass {
    if !immediate_wins(board, board.to_move()).is_empty() {
        TacticalClass::Win
    } else if !forced_blocks(board).is_empty() {
        TacticalClass::Block
    } else {
        TacticalClass::Quiet
    }
}

/// Provide a policy target and value target for the board
pub fn label(board: &Board) -> (Vec<(Move, f32)>, f32) {
    let legal: Vec<Move> = board.empty_moves().collect();
    if legal.is_empty() {
        return (Vec::new(), 0.0);
    }

    let wins: MoveSet = immediate_wins(board, board.to_move());
    if !wins.is_empty() {
        return shape(&legal, wins, 1.0);
    }

    let blocks = forced_blocks(board);
    if !blocks.is_empty() {
        return shape(&legal, blocks, -1.0);
    }

    let uniform = 1.0 / legal.len() as f32;
    let policy: Vec<(Move, f32)> = legal.iter().map(|&mv| (mv, uniform)).collect();
    (policy, 0.0)
}

fn shape(legal: &[Move], tactical: MoveSet, value: f32) -> (Vec<(Move, f32)>, f32) {
    let k = tactical.len() as usize;
    let n = legal.len();
    let rest = n - k;

    let tactical_p = 0.9f32 / k as f32;
    let other_p = 0.1_f32 / rest as f32;

    let policy: Vec<(Move, f32)> = legal
        .iter()
        .map(|&mv| {
            if tactical.contains(mv) {
                (mv, tactical_p)
            } else {
                (mv, other_p)
            }
        })
        .collect();

    (policy, value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::reference::{board_from_ascii, draw_game_moves};

    #[test]
    fn open_four_for_side_to_move_is_win() {
        let board = board_from_ascii(
            "
            O . O . O . O . . . . . . . .
            . . . . X X X X . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            ",
        );

        assert_eq!(classify(&board), TacticalClass::Win);
        let (policy, value) = label(&board);
        assert_eq!(value, 1.0);

        let legal_count = board.empty_moves().count();
        assert_eq!(policy.len(), legal_count);

        let wins = immediate_wins(&board, board.to_move());

        let sum: f32 = policy.iter().map(|(_, p)| p).sum();
        assert!(
            (sum - 1.0).abs() < 1e-5,
            "Policy masses should sum to {sum}"
        );

        assert_policy_represents_move_set(policy, wins, legal_count);
    }

    #[test]
    fn closed_four_for_opponent_is_block() {
        let board = board_from_ascii(
            "
            O . O . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . O X X X X . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            ",
        );

        assert_eq!(classify(&board), TacticalClass::Block);
        let (policy, value) = label(&board);
        assert_eq!(value, -1.0);

        let legal_count = board.empty_moves().count();
        assert_eq!(legal_count, policy.len());

        let blocks = forced_blocks(&board);

        let sum: f32 = policy.iter().map(|(_, p)| p).sum();
        assert!((sum - 1.0).abs() < 1e-5, "Policy masses sum to {sum}");

        assert_policy_represents_move_set(policy, blocks, legal_count);
    }

    fn assert_policy_represents_move_set(
        policy: Vec<(Move, f32)>,
        move_set: MoveSet,
        legal_count: usize,
    ) {
        let argmax = policy
            .iter()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(m, _)| *m)
            .unwrap();

        assert!(
            move_set.contains(argmax),
            "argmax {argmax:?} is a forced block."
        );

        let k = move_set.len() as usize;
        let rest = legal_count - k;

        for (mv, p) in &policy {
            if move_set.contains(*mv) {
                assert!((p - 0.9 / k as f32).abs() < 1e-5);
            } else {
                assert!((p - 0.1 / rest as f32).abs() < 1e-5);
            }
        }
    }

    #[test]
    fn scattered_position_is_quiet_and_uniform() {
        let board = board_from_ascii(
            "
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . X . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . O . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . O . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . X . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            ",
        );
        assert_eq!(classify(&board), TacticalClass::Quiet);
        let (policy, value) = label(&board);
        assert_eq!(value, 0.0);

        let legal_count = board.empty_moves().count();
        assert_eq!(legal_count, policy.len());

        let sum: f32 = policy.iter().map(|(_, p)| p).sum();
        assert!((sum - 1.0).abs() < 1e-5, "Policy masses sum to {sum}");

        let expected = 1.0 / legal_count as f32;
        for (_, p) in &policy {
            assert!((p - expected).abs() < 1e-5);
        }
    }

    #[test]
    fn win_takes_precedence_when_both_sides_have_immediate_win() {
        let board = board_from_ascii(
            "
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . X X X X . . . . . . . .
            . . . O O O O . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            ",
        );
        let wins = immediate_wins(&board, board.to_move());
        let opponent_wins = immediate_wins(&board, board.to_move().other());
        assert!(!wins.is_empty(), "Side to move has immediate win.");
        assert!(!opponent_wins.is_empty(), "Opponent could also win.");

        assert_eq!(classify(&board), TacticalClass::Win);
        let (policy, value) = label(&board);
        assert_eq!(value, 1.0);

        let k = wins.len() as usize;
        for (mv, p) in &policy {
            if wins.contains(*mv) {
                assert!((p - 0.9 / k as f32).abs() < 1e-5);
            } else {
                assert!(
                    *p < 0.9 / k as f32,
                    "Non-tactical mass is below tactical mass."
                )
            }
        }
    }

    #[test]
    fn full_board_terminal_dra_has_empty_policy_and_zero_value() {
        let mut board = Board::new();
        for mv in draw_game_moves() {
            board.play(mv).unwrap();
        }
        assert_eq!(board.status(), engine::Status::Draw);
        assert!(board.empty_moves().next().is_none());

        let (policy, value) = label(&board);
        assert!(policy.is_empty());
        assert_eq!(value, 0.0);
    }
}
