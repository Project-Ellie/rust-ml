//! Heatmap rendering for the `patterndemo` binary.
//!
//! This module is kept as a small library so the CLI's `main()` stays
//! thin and the map-selection / rendering logic can be unit-tested.

use crate::net::ThreatMaps;
use engine::{Board, Color, Move};

/// Supported threat maps that the demo can overlay on the board.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MapName {
    /// Immediate wins.
    Wins,
    /// Double-threat moves (open four / double four).
    DoubleThreats,
    /// Double-three fork moves.
    #[default]
    DoubleThrees,
    /// Open-three makers in the horizontal direction.
    ThreesH,
    /// Open-three makers in the vertical direction.
    ThreesV,
    /// Open-three makers in the diagonal-down direction.
    ThreesDd,
    /// Open-three makers in the diagonal-up direction.
    ThreesDu,
    /// The sparse threat potential (policy-shaped output).
    Policy,
}

impl MapName {
    /// Parse a map name from a CLI argument.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "wins" => Some(MapName::Wins),
            "double_threats" => Some(MapName::DoubleThreats),
            "double_threes" => Some(MapName::DoubleThrees),
            "threes_h" => Some(MapName::ThreesH),
            "threes_v" => Some(MapName::ThreesV),
            "threes_dd" => Some(MapName::ThreesDd),
            "threes_du" => Some(MapName::ThreesDu),
            "policy" => Some(MapName::Policy),
            _ => None,
        }
    }

    /// Human-readable label for the map.
    pub fn label(self) -> &'static str {
        match self {
            MapName::Wins => "immediate wins",
            MapName::DoubleThreats => "double threats",
            MapName::DoubleThrees => "double-three forks",
            MapName::ThreesH => "open-three makers (horizontal)",
            MapName::ThreesV => "open-three makers (vertical)",
            MapName::ThreesDd => "open-three makers (diag down)",
            MapName::ThreesDu => "open-three makers (diag up)",
            MapName::Policy => "threat potential (policy)",
        }
    }
}

/// Supported built-in showcase positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShowcaseName {
    /// Plus-sign double-three fork through the centre.
    #[default]
    DoubleThreeFork,
    /// Classic double-threat fork from the engine tactics tests.
    ClassicFork,
    /// Corner double-threat position from the engine tactics tests.
    CornerFork,
    /// A quiet position with no tactics.
    Quiet,
}

impl ShowcaseName {
    /// Parse a showcase name from a CLI argument.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "double-three-fork" => Some(ShowcaseName::DoubleThreeFork),
            "classic-fork" => Some(ShowcaseName::ClassicFork),
            "corner-fork" => Some(ShowcaseName::CornerFork),
            "quiet" => Some(ShowcaseName::Quiet),
            _ => None,
        }
    }

    /// Human-readable label for the showcase.
    pub fn label(self) -> &'static str {
        match self {
            ShowcaseName::DoubleThreeFork => "double-three fork",
            ShowcaseName::ClassicFork => "classic fork",
            ShowcaseName::CornerFork => "corner fork",
            ShowcaseName::Quiet => "quiet position",
        }
    }
}

/// Return one of the built-in showcase boards.
///
/// The boards are adapted from the engine tactics tests and the crate's
/// own three-map tests. Black is the attacking side unless the board
/// says otherwise.
pub fn showcase_board(name: ShowcaseName) -> Board {
    let mut b = Board::new();
    match name {
        ShowcaseName::DoubleThreeFork => {
            play_sequence(
                &mut b,
                &[
                    (Color::Black, 6, 7),
                    (Color::White, 0, 0),
                    (Color::Black, 7, 6),
                    (Color::White, 0, 14),
                    (Color::Black, 7, 8),
                    (Color::White, 14, 0),
                    (Color::Black, 8, 7),
                    (Color::White, 14, 14),
                ],
            );
        }
        ShowcaseName::ClassicFork => {
            play_sequence(
                &mut b,
                &[
                    (Color::Black, 4, 7),
                    (Color::White, 0, 14),
                    (Color::Black, 5, 7),
                    (Color::White, 3, 7),
                    (Color::Black, 6, 7),
                    (Color::White, 7, 3),
                    (Color::Black, 7, 4),
                    (Color::White, 11, 10),
                    (Color::Black, 7, 5),
                    (Color::White, 14, 0),
                    (Color::Black, 7, 6),
                    (Color::White, 14, 14),
                ],
            );
        }
        ShowcaseName::CornerFork => {
            play_sequence(
                &mut b,
                &[
                    (Color::Black, 0, 1),
                    (Color::White, 14, 14),
                    (Color::Black, 0, 2),
                    (Color::White, 14, 12),
                    (Color::Black, 0, 3),
                    (Color::White, 14, 10),
                    (Color::Black, 1, 0),
                    (Color::White, 14, 8),
                    (Color::Black, 2, 0),
                    (Color::White, 14, 6),
                    (Color::Black, 3, 0),
                    (Color::White, 14, 4),
                ],
            );
        }
        ShowcaseName::Quiet => {
            play_sequence(
                &mut b,
                &[
                    (Color::Black, 2, 2),
                    (Color::White, 5, 9),
                    (Color::Black, 12, 12),
                    (Color::White, 9, 5),
                ],
            );
        }
    }
    b
}

/// Replay a sequence of alternating moves on a fresh board.
///
/// Showcases are built by playing legal moves rather than by calling
/// `engine::reference::board_from_ascii` because `board_from_ascii` lives
/// behind the engine's `testutil` feature and must not leak into non-test
/// or binary code.
fn play_sequence(b: &mut Board, seq: &[(Color, u8, u8)]) {
    for &(side, r, c) in seq {
        assert_eq!(b.to_move(), side, "showcase move order mismatch");
        b.play(Move::new(r, c).expect("in-board coordinates"))
            .expect("showcase moves are legal");
    }
}

/// The side to evaluate for a given showcase board.
///
/// All showcase positions are constructed around Black threats.
pub fn showcase_side(_name: ShowcaseName) -> Color {
    Color::Black
}

/// Extract the scalar value for a cell from the selected map.
pub fn map_value(maps: &ThreatMaps, map: MapName, r: usize, c: usize) -> f32 {
    match map {
        MapName::Wins => maps.wins[r][c],
        MapName::DoubleThreats => maps.double_threats[r][c],
        MapName::DoubleThrees => maps.double_threes[r][c],
        MapName::ThreesH => maps.threes_per_dir[0][r][c],
        MapName::ThreesV => maps.threes_per_dir[1][r][c],
        MapName::ThreesDd => maps.threes_per_dir[2][r][c],
        MapName::ThreesDu => maps.threes_per_dir[3][r][c],
        MapName::Policy => crate::potential::score_cell(maps, r, c),
    }
}

/// Render a 15×15 board plus the selected threat map as an ANSI string.
///
/// Each cell is two characters wide. Stones are shown as `● ` for Black
/// and `○ ` for White. Empty cells display the map value (or `.` for
/// zero) on a coloured background whose intensity corresponds to the
/// score.
pub fn render_heatmap(
    maps: &ThreatMaps,
    board: &Board,
    showcase: ShowcaseName,
    map: MapName,
    backend: &str,
) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "Showcase: {} | Map: {} | Backend: {}",
        showcase.label(),
        map.label(),
        backend
    ));
    lines.push("    0 1 2 3 4 5 6 7 8 9 10 11 12 13 14".to_string());

    for r in 0..15u8 {
        let mut row = format!("{:>2}  ", r);
        for c in 0..15u8 {
            row.push_str(&cell(maps, board, map, r as usize, c as usize));
        }
        lines.push(row);
    }

    lines.push(String::new());
    lines.push(legend(map));
    lines.join("\n")
}

fn cell(maps: &ThreatMaps, board: &Board, map: MapName, r: usize, c: usize) -> String {
    let mv = Move::new(r as u8, c as u8).unwrap();
    match board.stone_at(mv) {
        Some(Color::Black) => black_stone(),
        Some(Color::White) => white_stone(),
        None => {
            let v = map_value(maps, map, r, c);
            if v > 0.0 {
                scored_empty(v)
            } else {
                " .".to_string()
            }
        }
    }
}

fn black_stone() -> String {
    " ●".to_string()
}

fn white_stone() -> String {
    " ○".to_string()
}

fn scored_empty(value: f32) -> String {
    let (bg, text) = color_for(value);
    format!("\x1b[48;5;{}m{} \x1b[0m", bg, text)
}

/// Map a score to an ANSI 256-colour background and a one-character label.
fn color_for(value: f32) -> (u8, char) {
    // Discrete policy levels are coloured distinctly; for binary maps
    // (value == 1.0) the chosen colour still works.
    match (value * 10.0).round() as i32 {
        10 => (196, '1'), // win
        9 => (208, '9'),  // double threat
        8 => (220, '8'),  // double three
        3 => (39, '3'),   // single three
        _ => (250, '*'),
    }
}

fn legend(map: MapName) -> String {
    match map {
        MapName::Policy => {
            "Legend: 1=win (1.0)  9=double threat (0.9)  8=double three (0.8)  3=three maker (0.3)"
                .to_string()
        }
        _ => "Legend: ● Black  ○ White  highlighted = map value > 0".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use burn::tensor::Device;
    use engine::{Color, Move};

    #[test]
    fn map_name_parsing() {
        assert_eq!(MapName::parse("wins"), Some(MapName::Wins));
        assert_eq!(
            MapName::parse("double_threats"),
            Some(MapName::DoubleThreats)
        );
        assert_eq!(MapName::parse("double_threes"), Some(MapName::DoubleThrees));
        assert_eq!(MapName::parse("threes_h"), Some(MapName::ThreesH));
        assert_eq!(MapName::parse("threes_v"), Some(MapName::ThreesV));
        assert_eq!(MapName::parse("threes_dd"), Some(MapName::ThreesDd));
        assert_eq!(MapName::parse("threes_du"), Some(MapName::ThreesDu));
        assert_eq!(MapName::parse("policy"), Some(MapName::Policy));
        assert_eq!(MapName::parse("unknown"), None);
    }

    #[test]
    fn showcase_name_parsing() {
        assert_eq!(
            ShowcaseName::parse("double-three-fork"),
            Some(ShowcaseName::DoubleThreeFork)
        );
        assert_eq!(
            ShowcaseName::parse("classic-fork"),
            Some(ShowcaseName::ClassicFork)
        );
        assert_eq!(
            ShowcaseName::parse("corner-fork"),
            Some(ShowcaseName::CornerFork)
        );
        assert_eq!(ShowcaseName::parse("quiet"), Some(ShowcaseName::Quiet));
        assert_eq!(ShowcaseName::parse("nope"), None);
    }

    #[test]
    fn classic_fork_is_exactly_one_double_threat_at_centre() {
        use crate::net::{ThreatNet, analyze};

        let b = showcase_board(ShowcaseName::ClassicFork);
        assert_eq!(b.stone_at(Move::new(4, 7).unwrap()), Some(Color::Black));
        assert_eq!(b.stone_at(Move::new(5, 7).unwrap()), Some(Color::Black));
        assert_eq!(b.stone_at(Move::new(6, 7).unwrap()), Some(Color::Black));
        assert_eq!(b.stone_at(Move::new(7, 4).unwrap()), Some(Color::Black));
        assert_eq!(b.stone_at(Move::new(7, 5).unwrap()), Some(Color::Black));
        assert_eq!(b.stone_at(Move::new(7, 6).unwrap()), Some(Color::Black));

        let device = Device::flex();
        let net = ThreatNet::new(&device);
        let maps = analyze(&net, &b, Color::Black, &device);

        let mut double_threat_cells = Vec::new();
        for r in 0..15 {
            for c in 0..15 {
                if maps.is_double_threat(r, c) {
                    double_threat_cells.push((r, c));
                }
            }
        }
        assert_eq!(double_threat_cells, vec![(7, 7)]);
    }

    #[test]
    fn double_three_fork_has_fork_at_centre() {
        use crate::net::{ThreatNet, analyze};

        let b = showcase_board(ShowcaseName::DoubleThreeFork);
        assert_eq!(b.stone_at(Move::new(7, 7).unwrap()), None);
        assert_eq!(b.stone_at(Move::new(6, 7).unwrap()), Some(Color::Black));
        assert_eq!(b.stone_at(Move::new(7, 6).unwrap()), Some(Color::Black));
        assert_eq!(b.stone_at(Move::new(7, 8).unwrap()), Some(Color::Black));
        assert_eq!(b.stone_at(Move::new(8, 7).unwrap()), Some(Color::Black));

        let device = Device::flex();
        let net = ThreatNet::new(&device);
        let maps = analyze(&net, &b, Color::Black, &device);
        assert!(maps.is_double_three(7, 7));
    }

    #[test]
    fn corner_fork_has_black_stones_in_corner() {
        let b = showcase_board(ShowcaseName::CornerFork);
        assert_eq!(b.stone_at(Move::new(0, 1).unwrap()), Some(Color::Black));
        assert_eq!(b.stone_at(Move::new(1, 0).unwrap()), Some(Color::Black));
    }

    #[test]
    fn quiet_position_has_no_black_tactics() {
        let b = showcase_board(ShowcaseName::Quiet);
        assert!(engine::immediate_wins(&b, Color::Black).is_empty());
        assert!(engine::double_threats(&b, Color::Black).is_empty());
    }

    #[test]
    fn render_includes_title_and_backend() {
        let maps = ThreatMaps {
            wins: [[0.0; 15]; 15],
            double_threats: [[0.0; 15]; 15],
            double_threes: [[0.0; 15]; 15],
            threes_per_dir: [[[0.0; 15]; 15]; 4],
            fours_created_per_dir: [[[0.0; 15]; 15]; 4],
        };
        let board = Board::new();
        let out = render_heatmap(
            &maps,
            &board,
            ShowcaseName::Quiet,
            MapName::DoubleThrees,
            "flex",
        );
        assert!(out.contains("Map: double-three forks"));
        assert!(out.contains("Backend: flex"));
        assert!(out.contains("●"));
    }
}
