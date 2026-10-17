//! The hand-written threat network and the win map.
//!
//! The network is a tiny two-layer convolutional module:
//!
//! 1. Layer 1 (`Conv2d`, 11×11, 2 → 152 channels, valid padding, ReLU)
//!    evaluates every pattern kernel from [`crate::kernel`] at every
//!    board cell.
//! 2. Layer 2 (`Conv2d`, 1×1, 152 → 9 channels, ReLU) is a
//!    cross-channel combiner: it ORs the five-completer channels into
//!    a win map, sums the four-maker channels per direction (scaling
//!    open fours by 2), and sums the three-maker channels per
//!    direction.
//!
//! Every map is read at *empty* cells only: the kernel language does
//! not condition on the candidate cell itself, so the raw network
//! output fires on occupied cells as well. The win-map glue zeroes
//! them, matching `engine::immediate_wins`, which only ever returns
//! empty cells.

use crate::kernel::{
    Direction, LAYER1_NUM_CHANNELS, LAYER2_NUM_CHANNELS, PatternFamily, RADIUS, layer1_conv,
    layer2_conv,
};
use crate::planes::{PADDED, Planes, planes};
use burn::module::Module;
use burn::nn::Relu;
use burn::nn::conv::Conv2d;
use burn::tensor::{Device, Tensor, TensorData};
use engine::{Board, Color, Move};

/// The hand-written threat network.
///
/// The type has no backend type parameter; build it on
/// `Device::flex()` (default) or `Device::wgpu(...)` (with the `gpu`
/// feature).
#[derive(Debug, Module)]
pub struct ThreatNet {
    layer1: Conv2d,
    relu1: Relu,
    layer2: Conv2d,
    relu2: Relu,
}

impl ThreatNet {
    /// Build a fresh network with hand-manufactured weights on `device`.
    pub fn new(device: &Device) -> Self {
        Self {
            layer1: layer1_conv(device),
            relu1: Relu::new(),
            layer2: layer2_conv(device),
            relu2: Relu::new(),
        }
    }

    /// Forward pass. Input shape `[batch, 2, 25, 25]`; output shape
    /// `[batch, 9, 15, 15]`.
    ///
    /// Output channel layout:
    /// * `0` — win map,
    /// * `1..=4` — five-completing cells created per direction,
    /// * `5..=8` — open-three makers per direction.
    pub fn forward(&self, input: Tensor<4>) -> Tensor<4> {
        let (_l1, l2) = self.forward_intermediate(input);
        l2
    }

    /// Forward pass returning both the layer-1 ReLU activations and
    /// the final layer-2 output.
    ///
    /// Layer-1 activations are exposed for chapter 4: the
    /// double-threat glue must count *unique* five-completing cells,
    /// which layer 2's per-direction sums cannot express.
    pub fn forward_intermediate(&self, input: Tensor<4>) -> (Tensor<4>, Tensor<4>) {
        let x = self.layer1.forward(input);
        let l1 = self.relu1.forward(x);
        let x = self.layer2.forward(l1.clone());
        let l2 = self.relu2.forward(x);
        (l1, l2)
    }

    /// The win map for `side` on `board`: a 15×15 grid that is
    /// positive exactly at empty cells where placing a stone
    /// completes five immediately.
    pub fn win_map(&self, board: &Board, side: Color, device: &Device) -> [[f32; 15]; 15] {
        let input = planes_tensor(board, side, device);
        let out = self.forward(input);
        let values: Vec<f32> = out.to_data().iter::<f32>().collect();

        let mut map = [[0.0f32; 15]; 15];
        for (r, row) in map.iter_mut().enumerate() {
            for (c, cell) in row.iter_mut().enumerate() {
                let mv = Move::new(r as u8, c as u8).expect("(r, c) is in-board");
                if board.stone_at(mv).is_none() {
                    // Channel 0 of the [1, 9, 15, 15] output.
                    *cell = values[r * 15 + c];
                }
            }
        }
        map
    }
}

/// The network's output maps restricted to the 15×15 in-board region.
///
/// Every map scores empty cells; occupied cells are zeroed by the glue
/// because the kernel language does not condition on the candidate
/// cell `c` itself.
#[derive(Debug, PartialEq)]
pub struct ThreatMaps {
    /// Cells where playing completes Five immmediately
    pub wins: [[f32; 15]; 15],
    /// Cells where playing leaves two or more five-completing cells
    pub double_threats: [[f32; 15]; 15],
    /// Cells where playing creates open threes in two or more directions
    pub double_threes: [[f32; 15]; 15],
    /// Per-direction open-three maker maps.
    ///
    /// Indexed `[direction][row][col]` where direction order is
    /// horizontal, vertical, diagonal down-right, diagonal up-right.
    pub threes_per_dir: [[[f32; 15]; 15]; 4],
    /// Per-direction count of five-completing cells created by the
    /// candidate move.
    ///
    /// This is a raw per-pattern sum (open four = 2, broken four = 1);
    /// it may double-count a single completing cell that is produced by
    /// two windows. It is kept for visualisation; [`ThreatMaps::double_threats`]
    /// uses the unique-cell count instead.
    ///
    /// Indexed `[direction][row][col]` where direction order is
    /// horizontal, vertical, diagonal down-right, diagonal up-right.
    pub fours_created_per_dir: [[[f32; 15]; 15]; 4],
}

impl ThreatMaps {
    /// True if the cell is marked as immediate win.
    pub fn is_win(&self, r: usize, c: usize) -> bool {
        self.wins[r][c] > 0.0
    }

    /// True iff the cell is marked as a double threat.
    pub fn is_double_threat(&self, r: usize, c: usize) -> bool {
        self.double_threats[r][c] > 0.0
    }

    /// True iff the cell is marked as a double-three fork.
    pub fn is_double_three(&self, r: usize, c: usize) -> bool {
        self.double_threes[r][c] > 0.0
    }
}

/// Analyze board `b` for side `s` and return all threat maps.
///
/// This evaluates a single colour; call twice (or use
/// [`analyze_both`]) to obtain maps for both sides.
pub fn analyze(net: &ThreatNet, b: &Board, s: Color, device: &Device) -> ThreatMaps {
    let input = planes_tensor(b, s, device);
    let (l1, l2) = net.forward_intermediate(input);
    maps_from_output(b, s, l1, l2, 0)
}

/// Analyze board `b` for both colours in one batched forward pass.
///
/// Returns `[black_maps, white_maps]`.
pub fn analyze_both(net: &ThreatNet, b: &Board, device: &Device) -> [ThreatMaps; 2] {
    let input = planes_tensor_both(b, device);
    let (l1, l2) = net.forward_intermediate(input);
    let black = maps_from_output(b, Color::Black, l1.clone(), l2.clone(), 0);
    let white = maps_from_output(b, Color::White, l1.clone(), l2.clone(), 1);

    [black, white]
}

/// Convert the padded planes for `side` into the network's NCHW input
/// tensor `[1, 2, 25, 25]` (channel 0 = own stones, channel 1 =
/// blocked cells).
fn planes_tensor(board: &Board, side: Color, device: &Device) -> Tensor<4> {
    let Planes { stones, blocked } = planes(board, side);
    let mut data = Vec::with_capacity(2 * PADDED * PADDED);
    data.extend_from_slice(&stones.concat());
    data.extend_from_slice(&blocked.concat());
    Tensor::<4>::from_data(TensorData::new(data, [1, 2, PADDED, PADDED]), device)
}

fn planes_tensor_both(board: &Board, device: &Device) -> Tensor<4> {
    let black = planes(board, Color::Black);
    let white = planes(board, Color::White);
    let mut data = Vec::with_capacity(2 * 2 * PADDED * PADDED);
    data.extend_from_slice(&black.stones.concat());
    data.extend_from_slice(&black.blocked.concat());
    data.extend_from_slice(&white.stones.concat());
    data.extend_from_slice(&white.blocked.concat());

    Tensor::<4>::from_data(TensorData::new(data, [2, 2, PADDED, PADDED]), device)
}

fn maps_from_output(b: &Board, s: Color, l1: Tensor<4>, l2: Tensor<4>, batch: usize) -> ThreatMaps {
    let l1_data: Vec<f32> = l1.to_data().iter::<f32>().collect();
    let l2_data: Vec<f32> = l2.to_data().iter::<f32>().collect();
    maps_from_flat(b, s, &l1_data, &l2_data, batch)
}

/// Convert flat layer-1/2 outputs into [`ThreatMaps`].
///
/// Pipeline: extract NCHW layer-2 values → zero occupied cells
/// → add the `before` scalar for already-winning moves → count unique
/// newly-created completing cells across firing four-makers
/// → derive the final double-threat and double-three maps.
fn maps_from_flat(
    b: &Board,
    s: Color,
    l1_data: &[f32],
    l2_data: &[f32],
    batch: usize,
) -> ThreatMaps {
    let mut wins = [[0.0f32; 15]; 15];
    let mut fours_created_per_dir = [[[0.0f32; 15]; 15]; 4];
    let mut threes_per_dir = [[[0.0f32; 15]; 15]; 4];

    let l2_base = batch * LAYER2_NUM_CHANNELS * 15 * 15;
    for ch in 0..LAYER2_NUM_CHANNELS {
        for r in 0..15 {
            for c in 0..15 {
                let idx = l2_base + ((ch * 15) + r) * 15 + c;
                let v = l2_data[idx];
                match ch {
                    0 => wins[r][c] = v,
                    1..=4 => fours_created_per_dir[ch - 1][r][c] = v,
                    5..=8 => threes_per_dir[ch - 5][r][c] = v,
                    _ => unreachable!(),
                }
            }
        }
    }

    let before = engine::immediate_wins(b, s).len() as f32;
    let mut double_threats = [[0.0f32; 15]; 15];
    let mut double_threes = [[0.0f32; 15]; 15];

    let table = crate::kernel::pattern_table();

    for r in 0..15 {
        for c in 0..15 {
            let mv = Move::new(r as u8, c as u8).expect("in-board indices");
            let occupied = b.stone_at(mv).is_some();
            if occupied {
                // Threat maps are only defined for empty cells; the
                // kernel language does not condition on `c` itself.
                wins[r][c] = 0.0;
                for d in 0..4 {
                    fours_created_per_dir[d][r][c] = 0.0;
                    threes_per_dir[d][r][c] = 0.0;
                }
                continue;
            }

            // Unique five-completing cells *newly created* by playing
            // at (r,c). A completing cell that was already an immediate
            // win before the move belongs to `before`, not to the newly
            // created set, so we filter it out by checking the win map
            // at the completing cell's location.
            let mut created_count = 0usize;
            for dir in Direction::ALL {
                let mut seen = [false; (2 * RADIUS as usize + 1)];
                for (ch, entry) in table.iter().enumerate() {
                    if entry.direction != dir {
                        continue;
                    }
                    if !matches!(
                        entry.family,
                        PatternFamily::OpenFourMaker | PatternFamily::BrokenFourMaker
                    ) {
                        continue;
                    }
                    let l1_idx = ((batch * LAYER1_NUM_CHANNELS + ch) * 15 + r) * 15 + c;
                    if l1_data[l1_idx] > 0.0 {
                        for &(dy, dx) in &entry.completing_offsets {
                            let c2_r = r as i32 + dy;
                            let c2_c = c as i32 + dx;
                            if !(0..15).contains(&c2_r) || !(0..15).contains(&c2_c) {
                                continue;
                            }
                            let c2_r = c2_r as usize;
                            let c2_c = c2_c as usize;
                            if wins[c2_r][c2_c] > 0.0 {
                                // Already counted in `before`.
                                continue;
                            }
                            let offset = offset_along_direction(dy, dx, dir);
                            let arr_idx = (offset + RADIUS) as usize;
                            seen[arr_idx] = true;
                        }
                    }
                }
                created_count += seen.iter().filter(|&&x| x).count();
            }

            if wins[r][c] == 0.0 && created_count as f32 + before >= 2.0 {
                double_threats[r][c] = 1.0;
            }

            let mut three_sum = 0.0f32;
            for dir_map in &threes_per_dir {
                three_sum += dir_map[r][c];
            }
            // The pattern table never fires two three-makers for the same
            // cell in the same direction, so the sum counts distinct directions.
            if three_sum >= 2.0 {
                double_threes[r][c] = 1.0;
            }
        }
    }

    ThreatMaps {
        wins,
        double_threats,
        double_threes,
        threes_per_dir,
        fours_created_per_dir,
    }
}

/// Project a 2-D offset onto the 1-D coordinate along `dir`.
///
/// The returned value is the scalar `k` such that stepping `k` times
/// along `dir` from the origin reaches `(dy,dx)`.
fn offset_along_direction(dy: i32, dx: i32, dir: Direction) -> i32 {
    match dir {
        Direction::Horizontal => dx,
        Direction::Vertical => dy,
        Direction::DiagDown => dy,
        Direction::DiagUp => dx,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::MoveSet;

    /// Parse a 15-line ASCII board: `x` = black stone, `o` = white
    /// stone, anything else = empty. Black to move, so the position
    /// must have at least as many white stones as black stones.
    fn board_from_ascii(image: &str) -> Board {
        let mut black = Vec::new();
        let mut white = Vec::new();
        for (r, line) in image.trim().lines().enumerate() {
            for (c, ch) in line.chars().filter(|ch| !ch.is_whitespace()).enumerate() {
                let mv = Move::new(r as u8, c as u8).expect("(r, c) is in-board");
                match ch {
                    'x' => black.push(mv),
                    'o' => white.push(mv),
                    _ => {}
                }
            }
        }
        Board::from_position(&black, &white, Color::Black).expect("valid test position")
    }

    /// The win map must agree with `engine::immediate_wins` on every
    /// one of the 225 cells — exact set equality, both directions.
    fn assert_win_map_matches_engine(image: &str) {
        let board = board_from_ascii(image);
        let device = Device::flex();
        let net = ThreatNet::new(&device);
        let map = net.win_map(&board, Color::Black, &device);
        let wins = engine::immediate_wins(&board, Color::Black);

        for r in 0..15u8 {
            for c in 0..15u8 {
                let mv = Move::new(r, c).expect("(r, c) is in-board");
                assert_eq!(
                    map[r as usize][c as usize] > 0.0,
                    wins.contains(mv),
                    "win map disagrees with engine at ({r}, {c})"
                );
            }
        }
    }

    fn set_from_map(map: &[[f32; 15]; 15]) -> MoveSet {
        let mut s = MoveSet::EMPTY;
        for r in 0..15u8 {
            for c in 0..15u8 {
                if map[r as usize][c as usize] > 0.0 {
                    s.insert(Move::new(r, c).unwrap());
                }
            }
        }
        s
    }

    /// Assert that the network's `double_threats` map agrees with
    /// `engine::double_threats` on the given board for Black.
    fn assert_double_threats_match_engine(image: &str) {
        let board = board_from_ascii(image);
        let device = Device::flex();
        let net = ThreatNet::new(&device);
        let maps = analyze(&net, &board, Color::Black, &device);
        let expected = engine::double_threats(&board, Color::Black);
        assert_eq!(
            set_from_map(&maps.double_threats),
            expected,
            "double threat map disagrees with engine."
        )
    }

    #[test]
    fn open_four_win_map_matches_engine() {
        // Black has an open four on row 7 (cols 4..=7): both ends,
        // (7, 3) and (7, 8), complete five immediately.
        let image = "
            o o o o . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . x x x x . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .";
        assert_win_map_matches_engine(image);
    }

    #[test]
    fn broken_four_win_map_matches_engine() {
        // Black has a broken four on row 7: cols 4,5,6 and 8. Only the
        // gap (7, 7) completes five. The white stone at (7, 9) sits
        // right next to the window — an occupied neighbour must not
        // disturb the detector.
        let image = "
            o o o o . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . x x x . x o . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .";
        assert_win_map_matches_engine(image);
    }

    #[test]
    fn classic_fork_double_threat() {
        let image = "
            . . . . . . . . . . . . . . o
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . o . . . . . . .
            . . . . . . . x . . . . . . .
            . . . . . . . x . . . . . . .
            . . . . . . . x . . . . . . .
            . . . o x x x . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . o . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            o . . . . . . . . . . . . . o";
        assert_double_threats_match_engine(image);
    }

    #[test]
    fn open_four_maker_is_double_threat() {
        let image = "
            o . . . . . . . . . . . . . o
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . x x x . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            o . . . . . . . . . . . . . .";
        assert_double_threats_match_engine(image);
    }

    #[test]
    fn two_broken_fours_make_double_threat() {
        let image = "
            o . . . . . . . . . . . . . o
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            x . x . x . x . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            o . . . . . . . . . . . . . o";
        assert_double_threats_match_engine(image);
    }
}
