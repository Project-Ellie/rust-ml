# Chapter 07 — opt-in solution: `potential.rs`, `render.rs`, `patterndemo.rs`, `tests/gpu_port.rs`

Open this only if you have been stuck for more than twenty minutes, or after you have finished the chapter and want to compare your work against the verified reference. The implementations below are the complete files from the reference crate, quoted verbatim.

Each file is fenced with four backticks so that any triple-backtick examples inside the source (for instance the usage block in the demo binary) do not break the markdown fence.

## `src/potential.rs`

Threat potential and policy-shaped sparse vector.

````rust
//! Threat potential: turn the threat maps into a sparse policy vector.
//!
//! The maps already score every legal move, so a small combiner produces
//! `policy(b, s) -> Vec<(Move, f32)>` — the same sparse shape as the
//! datagen tutorial's `Sample::policy` and the future mcts `Evaluator`
//! output. This is a *shape* contract for future integration, not an
//! integration itself.
//!
//! # Combination weights (`[experiment]`)
//!
//! The following weights are documented as experimental hyper-parameters.
//! They encode the threat ladder: a win now is unbeatable, a double
//! threat is next, a double-three fork is a promising sequence starter,
//! and a single open-three maker is the weakest signal. Cells that do
//! not fall into any category are omitted.
//!
//! | Category | Condition | Score |
//! |----------|-----------|-------|
//! | Win | `wins[r][c] > 0` | `1.0` |
//! | Double threat | `double_threats[r][c] > 0` | `0.9` |
//! | Double-three fork | `double_threes[r][c] > 0` | `0.8` |
//! | Single three maker | any `threes_per_dir[d][r][c] > 0` | `0.3` |
//!
//! A cell receives exactly one score: the highest-priority category it
//! satisfies. The result is sorted lexicographically by row then column so
//! the order is deterministic and matches the natural board scan.

use crate::net::{ThreatMaps, ThreatNet, analyze};
use burn::tensor::backend::Backend;
use engine::{Board, Color, Move};

/// Score assigned to an immediate winning move.
pub const WIN_SCORE: f32 = 1.0;

/// Score assigned to a double-threat move (open four or equivalent).
pub const DOUBLE_THREAT_SCORE: f32 = 0.9;

/// Score assigned to a double-three fork move.
pub const DOUBLE_THREE_SCORE: f32 = 0.8;

/// Score assigned to a single open-three maker move.
pub const THREE_SCORE: f32 = 0.3;

/// Build a sparse policy vector from the threat network.
///
/// The returned vector contains one entry for every legal empty in-board
/// cell that satisfies at least one threat category. Occupied cells and
/// cells with score `0.0` are omitted, so the vector may be empty.
///
/// The scores are chosen so that higher tactical value always dominates
/// lower value: a cell is never represented by more than one score.
pub fn policy<B: Backend>(
    net: &ThreatNet<B>,
    b: &Board,
    s: Color,
    device: &B::Device,
) -> Vec<(Move, f32)> {
    let maps = analyze(net, b, s, device);
    policy_from_maps(b, &maps)
}

/// Build a sparse policy vector from already-computed [`ThreatMaps`].
///
/// This is useful when the maps have already been computed for
/// visualisation or for comparing backends on the same position.
pub fn policy_from_maps(b: &Board, maps: &ThreatMaps) -> Vec<(Move, f32)> {
    let mut out = Vec::new();

    for r in 0..15u8 {
        for c in 0..15u8 {
            let mv = Move::new(r, c).expect("in-board indices");
            if b.stone_at(mv).is_some() {
                continue;
            }

            let score = score_cell(maps, r as usize, c as usize);
            if score > 0.0 {
                out.push((mv, score));
            }
        }
    }

    // Deterministic lexicographic order by row, then column (row-major scan).
    out.sort_by_key(|&(mv, _)| (mv.row(), mv.col()));
    out
}

/// Return the priority score for a single empty cell.
///
/// The first matching category wins; a cell receives at most one score.
pub(crate) fn score_cell(maps: &ThreatMaps, r: usize, c: usize) -> f32 {
    if maps.wins[r][c] > 0.0 {
        return WIN_SCORE;
    }
    if maps.double_threats[r][c] > 0.0 {
        return DOUBLE_THREAT_SCORE;
    }
    if maps.double_threes[r][c] > 0.0 {
        return DOUBLE_THREE_SCORE;
    }
    if maps.threes_per_dir.iter().any(|dir| dir[r][c] > 0.0) {
        return THREE_SCORE;
    }
    0.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use burn::backend::NdArray;
    use engine::{Color, Move, reference::board_from_ascii};

    #[test]
    fn double_three_fork_scores_zero_point_eight() {
        // Four black stones plus four white filler stones to keep the
        // board_from_ascii stone counts valid.
        let b = board_from_ascii(
            "
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . X . . . . . . .
            . . . . . . X . X . . . . . .
            . . . . . . . X . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . O
            O . . . . . . . . . . . . . O
            ",
        );
        let device = Default::default();
        let net = ThreatNet::<NdArray>::new(&device);
        let p = policy(&net, &b, Color::Black, &device);

        assert!(
            p.iter()
                .any(|(mv, s)| { (mv.row(), mv.col()) == (7, 7) && *s == DOUBLE_THREE_SCORE })
        );
    }

    #[test]
    fn open_four_maker_scores_zero_point_nine() {
        let b = board_from_ascii(
            "
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . X X X . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . O
            O . . . . . . . . . . . . . O
            ",
        );
        let device = Default::default();
        let net = ThreatNet::<NdArray>::new(&device);
        let p = policy(&net, &b, Color::Black, &device);

        assert!(
            p.iter()
                .any(|(mv, s)| { (mv.row(), mv.col()) == (7, 4) && *s == DOUBLE_THREAT_SCORE })
        );
        assert!(
            p.iter()
                .any(|(mv, s)| { (mv.row(), mv.col()) == (7, 8) && *s == DOUBLE_THREAT_SCORE })
        );
    }

    #[test]
    fn win_scores_one_point_zero() {
        let b = board_from_ascii(
            "
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . X X X X . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . O
            O . . . . . . . . . . . . . O
            ",
        );
        let device = Default::default();
        let net = ThreatNet::<NdArray>::new(&device);
        let p = policy(&net, &b, Color::Black, &device);

        assert!(
            p.iter()
                .any(|(mv, s)| { (mv.row(), mv.col()) == (7, 3) && *s == WIN_SCORE })
        );
        assert!(
            p.iter()
                .any(|(mv, s)| { (mv.row(), mv.col()) == (7, 8) && *s == WIN_SCORE })
        );
    }

    #[test]
    fn quiet_board_returns_empty_policy() {
        let b = board_from_ascii(
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
        let device = Default::default();
        let net = ThreatNet::<NdArray>::new(&device);
        let p = policy(&net, &b, Color::Black, &device);
        assert!(p.is_empty());
    }

    #[test]
    fn only_legal_empty_cells_are_present() {
        // Build a position where Black has an open four, then have White
        // block both winning cells through legal alternating play.
        let mut b = Board::new();
        let seq: &[(u8, u8, Color)] = &[
            (7, 4, Color::Black),
            (0, 0, Color::White),
            (7, 5, Color::Black),
            (0, 1, Color::White),
            (7, 6, Color::Black),
            (7, 3, Color::White), // block left winning cell
            (7, 7, Color::Black),
            (7, 8, Color::White), // block right winning cell
        ];
        for &(r, c, side) in seq {
            assert_eq!(b.to_move(), side, "play order mismatch");
            b.play(Move::new(r, c).unwrap()).unwrap();
        }

        let device = Default::default();
        let net = ThreatNet::<NdArray>::new(&device);
        let p = policy(&net, &b, Color::Black, &device);
        assert!(
            p.is_empty(),
            "occupied winning cells must not appear in policy"
        );
    }
}
````

## `src/render.rs`

Heatmap rendering and showcase boards.

````rust
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
    /// Open-three makers in one selected direction.
    ThreesH,
    /// Open-three makers in one selected direction.
    ThreesV,
    /// Open-three makers in one selected direction.
    ThreesDd,
    /// Open-three makers in one selected direction.
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
        use burn::backend::NdArray;

        let b = showcase_board(ShowcaseName::ClassicFork);
        assert_eq!(b.stone_at(Move::new(4, 7).unwrap()), Some(Color::Black));
        assert_eq!(b.stone_at(Move::new(5, 7).unwrap()), Some(Color::Black));
        assert_eq!(b.stone_at(Move::new(6, 7).unwrap()), Some(Color::Black));
        assert_eq!(b.stone_at(Move::new(7, 4).unwrap()), Some(Color::Black));
        assert_eq!(b.stone_at(Move::new(7, 5).unwrap()), Some(Color::Black));
        assert_eq!(b.stone_at(Move::new(7, 6).unwrap()), Some(Color::Black));

        let device = Default::default();
        let net = ThreatNet::<NdArray>::new(&device);
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
        use burn::backend::NdArray;

        let b = showcase_board(ShowcaseName::DoubleThreeFork);
        assert_eq!(b.stone_at(Move::new(7, 7).unwrap()), None);
        assert_eq!(b.stone_at(Move::new(6, 7).unwrap()), Some(Color::Black));
        assert_eq!(b.stone_at(Move::new(7, 6).unwrap()), Some(Color::Black));
        assert_eq!(b.stone_at(Move::new(7, 8).unwrap()), Some(Color::Black));
        assert_eq!(b.stone_at(Move::new(8, 7).unwrap()), Some(Color::Black));

        let device = Default::default();
        let net = ThreatNet::<NdArray>::new(&device);
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
            "NdArray",
        );
        assert!(out.contains("Map: double-three forks"));
        assert!(out.contains("Backend: NdArray"));
        assert!(out.contains("●"));
    }
}
````

## `src/bin/patterndemo.rs`

Terminal demo binary.

````rust
//! Terminal heatmap demo for the `patterns` threat network.
//!
//! Usage:
//!
//! ```text
//! cargo run -p patterns --bin patterndemo
//! cargo run -p patterns --bin patterndemo -- --map policy --showcase classic-fork
//! cargo run -p patterns --features gpu --bin patterndemo -- --gpu
//! ```
//!
//! The binary renders one of the built-in showcase boards with a selected
//! threat map overlaid as an ANSI-coloured heatmap. With the `gpu` feature
//! enabled, `--gpu` runs the analysis on the `Wgpu` backend.

use std::env;
use std::process;

use patterns::net::{ThreatNet, analyze};
use patterns::render::{MapName, ShowcaseName, render_heatmap, showcase_board, showcase_side};

use burn::backend::NdArray;
#[cfg(feature = "gpu")]
use burn::backend::Wgpu;

/// Command-line configuration for the demo.
#[derive(Debug, Clone, PartialEq)]
struct Args {
    /// Threat map to render.
    map: MapName,
    /// Built-in position to display.
    showcase: ShowcaseName,
    /// Whether to run on the GPU backend.
    gpu: bool,
}

fn main() {
    let raw: Vec<String> = env::args().skip(1).collect();
    if raw.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return;
    }

    let args = match parse_args(raw.into_iter()) {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("{msg}");
            eprintln!(
                "Usage: patterndemo [OPTIONS]\n\
                 \n\
                 Options:\n\
                 --map <NAME>        Threat map to render (default: double_threes)\n\
                 --showcase <NAME>   Built-in position (default: double-three-fork)\n\
                 --gpu               Run on the Wgpu backend (requires `gpu` feature)\n\
                 --help              Print this help message\n\
                 \n\
                 Valid maps: wins, double_threats, double_threes,\n\
                 threes_h, threes_v, threes_dd, threes_du, policy\n\
                 Valid showcases: double-three-fork, classic-fork, corner-fork, quiet"
            );
            process::exit(2);
        }
    };

    if let Err(e) = run(args) {
        eprintln!("patterndemo failed: {e}");
        process::exit(1);
    }
}

fn run(args: Args) -> Result<(), String> {
    let board = showcase_board(args.showcase);
    let side = showcase_side(args.showcase);

    if args.gpu {
        run_gpu(args.showcase, args.map, side, &board)
    } else {
        run_cpu(args.showcase, args.map, side, &board)
    }
}

#[cfg(feature = "gpu")]
fn run_gpu(
    showcase: ShowcaseName,
    map: MapName,
    side: engine::Color,
    board: &engine::Board,
) -> Result<(), String> {
    let device = Default::default();
    let net = ThreatNet::<Wgpu>::new(&device);
    let maps = analyze(&net, board, side, &device);
    print!("{}", render_heatmap(&maps, board, showcase, map, "Wgpu"));
    Ok(())
}

#[cfg(not(feature = "gpu"))]
fn run_gpu(
    _showcase: ShowcaseName,
    _map: MapName,
    _side: engine::Color,
    _board: &engine::Board,
) -> Result<(), String> {
    Err("--gpu requires the `gpu` feature (cargo run -p patterns --features gpu)".to_string())
}

fn run_cpu(
    showcase: ShowcaseName,
    map: MapName,
    side: engine::Color,
    board: &engine::Board,
) -> Result<(), String> {
    let device = Default::default();
    let net = ThreatNet::<NdArray>::new(&device);
    let maps = analyze(&net, board, side, &device);
    print!("{}", render_heatmap(&maps, board, showcase, map, "NdArray"));
    Ok(())
}

fn parse_args<I>(mut args: I) -> Result<Args, String>
where
    I: Iterator<Item = String>,
{
    let mut map = None;
    let mut showcase = None;
    let mut gpu = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--map" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--map requires a value".to_string())?;
                map = Some(MapName::parse(&value).ok_or_else(|| {
                    format!(
                        "unknown map '{value}'. Valid maps: wins, double_threats, double_threes, \
                         threes_h, threes_v, threes_dd, threes_du, policy"
                    )
                })?);
            }
            "--showcase" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--showcase requires a value".to_string())?;
                showcase = Some(ShowcaseName::parse(&value).ok_or_else(|| {
                    format!(
                        "unknown showcase '{value}'. Valid showcases: \
                         double-three-fork, classic-fork, corner-fork, quiet"
                    )
                })?);
            }
            "--gpu" => gpu = true,
            other => return Err(format!("unknown flag: {other}")),
        }
    }

    Ok(Args {
        map: map.unwrap_or_default(),
        showcase: showcase.unwrap_or_default(),
        gpu,
    })
}

fn print_help() {
    println!("patterndemo — render a Gomoku threat map as a terminal heatmap");
    println!();
    println!("Usage: patterndemo [OPTIONS]");
    println!();
    println!("Options:");
    println!("  --map <NAME>        Threat map to render (default: double_threes)");
    println!("                      wins | double_threats | double_threes |");
    println!("                      threes_h | threes_v | threes_dd | threes_du | policy");
    println!("  --showcase <NAME>   Built-in position (default: double-three-fork)");
    println!("                      double-three-fork | classic-fork | corner-fork | quiet");
    println!("  --gpu               Run on the Wgpu backend (requires `gpu` feature)");
    println!("  --help              Print this help message");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_args_rejects_unknown_flag() {
        let result = parse_args(["--nonsense".to_string()].into_iter());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown flag"));
    }

    #[test]
    fn parse_args_accepts_map_and_showcase() {
        let args = parse_args(
            [
                "--map".to_string(),
                "policy".to_string(),
                "--showcase".to_string(),
                "classic-fork".to_string(),
            ]
            .into_iter(),
        )
        .unwrap();

        assert_eq!(args.map, MapName::Policy);
        assert_eq!(args.showcase, ShowcaseName::ClassicFork);
        assert!(!args.gpu);
    }

    #[test]
    fn parse_args_parses_gpu_flag() {
        let args = parse_args(["--gpu".to_string()].into_iter()).unwrap();

        assert!(args.gpu);
        assert_eq!(args.map, MapName::default());
        assert_eq!(args.showcase, ShowcaseName::default());
    }
}
````

## `tests/gpu_port.rs`

CPU-vs-GPU portability test.

````rust
//! CPU-vs-GPU portability test.
//!
//! This test is only compiled when the `gpu` feature is enabled. It runs
//! the full `analyze` pipeline and the `policy()` combiner on both the
//! `NdArray` and `Wgpu` backends for a small set of fixed showcase boards
//! and asserts exact equality.
//!
//! Exact equality is valid because every kernel weight and every input
//! plane value is a small integer, so all `f32` sums are exact on both
//! backends.

#![cfg(feature = "gpu")]

use burn::backend::{NdArray, Wgpu};
use patterns::net::ThreatNet;
use patterns::potential::policy;
use patterns::render::{ShowcaseName, showcase_board, showcase_side};

fn assert_maps_equal_cpu_gpu(showcase: ShowcaseName) {
    let board = showcase_board(showcase);
    let side = showcase_side(showcase);

    let cpu_device = Default::default();
    let gpu_device = Default::default();

    let cpu_net = ThreatNet::<NdArray>::new(&cpu_device);
    let gpu_net = ThreatNet::<Wgpu>::new(&gpu_device);

    let cpu_maps = patterns::net::analyze(&cpu_net, &board, side, &cpu_device);
    let gpu_maps = patterns::net::analyze(&gpu_net, &board, side, &gpu_device);

    assert_eq!(
        cpu_maps, gpu_maps,
        "ThreatMaps must be identical on CPU and GPU for showcase {:?}",
        showcase
    );

    let cpu_policy = policy(&cpu_net, &board, side, &cpu_device);
    let gpu_policy = policy(&gpu_net, &board, side, &gpu_device);

    assert_eq!(
        cpu_policy, gpu_policy,
        "policy() must be identical on CPU and GPU for showcase {:?}",
        showcase
    );
}

#[test]
fn double_three_fork_cpu_gpu_equal() {
    assert_maps_equal_cpu_gpu(ShowcaseName::DoubleThreeFork);
}

#[test]
fn classic_fork_cpu_gpu_equal() {
    assert_maps_equal_cpu_gpu(ShowcaseName::ClassicFork);
}

#[test]
fn corner_fork_cpu_gpu_equal() {
    assert_maps_equal_cpu_gpu(ShowcaseName::CornerFork);
}

#[test]
fn quiet_position_cpu_gpu_equal() {
    assert_maps_equal_cpu_gpu(ShowcaseName::Quiet);
}
````

