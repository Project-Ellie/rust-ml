> **Opt-in solution — read only after you have tried the chapter, or if you have been stuck for more than twenty minutes.**
>
> This file quotes `src/net.rs` from the verified reference crate in
> its **final** form. The three-maker channels are still place-holders
> at this stage of the tutorial and are filled in chapters 5–6; the
> `kernels.rs` source was quoted in chapter 3. Quoting the finished
> file here lets you compare the shape of the `ThreatMaps` glue and the
> layer-2 integration without spoiling the incremental build.

````rust
//! Hand-written Burn threat network and `ThreatMaps` glue.
//!
//! The network is a tiny two-layer convolutional module:
//!
//! 1. Layer 1 (`Conv2d`, 11×11, 2 → 152 channels, valid padding, ReLU)
//!    evaluates every pattern kernel from [`crate::kernels`] at every
//!    board cell.
//! 2. Layer 2 (`Conv2d`, 1×1, 152 → 9 channels, ReLU) is a
//!    cross-channel combiner: it ORs the five-completer channels into
//!    a win map, sums the four-maker channels per direction (scaling
//!    open fours by 2), and sums the three-maker channels per
//!    direction.
//!
//! A small amount of Rust glue then turns the per-direction count maps
//! into the final `double_threats` and `double_threes` maps. For
//! `double_threats` the glue must count *unique* five-completing cells,
//! because the engine's `winning_cells` uses a `MoveSet`: one cell that
//! completes five via two different windows is counted once. The
//! network therefore exposes the layer-1 activations so the glue can
//! compute the union of completing cells from the firing open-four and
//! broken-four patterns.

use crate::kernels::{
    Direction, LAYER1_OUT, LAYER2_OUT, PatternFamily, RADIUS, layer1_conv, layer2_conv,
};
use crate::planes::{PADDED, Planes, planes};
use burn::module::Module;
use burn::nn::Relu;
use burn::nn::conv::Conv2d;
use burn::tensor::backend::Backend;
use burn::tensor::{Tensor, TensorData};
use engine::{Board, Color};

/// The hand-written threat network.
///
/// The type is backend-generic; instantiate it with `NdArray` (default)
/// or `Wgpu` (with the `gpu` feature).
#[derive(Debug, Module)]
pub struct ThreatNet<B: Backend> {
    layer1: Conv2d<B>,
    relu1: Relu,
    layer2: Conv2d<B>,
    relu2: Relu,
}

impl<B: Backend> ThreatNet<B> {
    /// Build a fresh network with hand-manufactured weights on `device`.
    pub fn new(device: &B::Device) -> Self {
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
    pub fn forward(&self, input: Tensor<B, 4>) -> Tensor<B, 4> {
        let (_l1, l2) = self.forward_intermediate(input);
        l2
    }

    /// Forward pass returning both the layer-1 ReLU activations and the
    /// final layer-2 output.
    ///
    /// Layer-1 activations are needed by the `double_threats` glue so
    /// it can count unique five-completing cells across firing pattern
    /// channels.
    pub fn forward_intermediate(&self, input: Tensor<B, 4>) -> (Tensor<B, 4>, Tensor<B, 4>) {
        let x = self.layer1.forward(input);
        let l1 = self.relu1.forward(x);
        let x = self.layer2.forward(l1.clone());
        let l2 = self.relu2.forward(x);
        (l1, l2)
    }
}

/// The network's output maps restricted to the 15×15 in-board region.
///
/// Every map scores empty cells; occupied cells are zeroed by the glue
/// because the kernel language does not condition on the candidate
/// cell `c` itself.
#[derive(Debug, Clone, PartialEq)]
pub struct ThreatMaps {
    /// Cells where playing completes five immediately.
    pub wins: [[f32; 15]; 15],
    /// Cells where playing leaves two or more five-completing cells.
    pub double_threats: [[f32; 15]; 15],
    /// Cells where playing creates open threes in two or more
    /// directions.
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
    /// two windows. It is kept for visualisation; `double_threats`
    /// uses the unique-cell count instead.
    ///
    /// Indexed `[direction][row][col]` where direction order is
    /// horizontal, vertical, diagonal down-right, diagonal up-right.
    pub fours_created_per_dir: [[[f32; 15]; 15]; 4],
}

impl ThreatMaps {
    /// True iff the cell is marked as an immediate win.
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
pub fn analyze<B: Backend>(
    net: &ThreatNet<B>,
    b: &Board,
    s: Color,
    device: &B::Device,
) -> ThreatMaps {
    let planes = planes(b, s);
    let input = planes_to_tensor(&planes, device);
    let (l1, l2) = net.forward_intermediate(input);
    maps_from_output(b, s, l1, l2, 0)
}

/// Analyze board `b` for both colours in one batched forward pass.
///
/// Returns `[black_maps, white_maps]`.
pub fn analyze_both<B: Backend>(
    net: &ThreatNet<B>,
    b: &Board,
    device: &B::Device,
) -> [ThreatMaps; 2] {
    let input = build_batched_input(b, device);
    let (l1, l2) = net.forward_intermediate(input);
    let black = maps_from_output(b, Color::Black, l1.clone(), l2.clone(), 0);
    let white = maps_from_output(b, Color::White, l1, l2, 1);
    [black, white]
}

fn build_batched_input<B: Backend>(b: &Board, device: &B::Device) -> Tensor<B, 4> {
    let black_planes = planes(b, Color::Black);
    let white_planes = planes(b, Color::White);

    let mut input_data = Vec::with_capacity(2 * 2 * PADDED * PADDED);
    for row in black_planes.stones {
        input_data.extend(row);
    }
    for row in black_planes.blocked {
        input_data.extend(row);
    }
    for row in white_planes.stones {
        input_data.extend(row);
    }
    for row in white_planes.blocked {
        input_data.extend(row);
    }

    Tensor::<B, 4>::from_data(TensorData::new(input_data, [2, 2, PADDED, PADDED]), device)
}

fn planes_to_tensor<B: Backend>(planes: &Planes, device: &B::Device) -> Tensor<B, 4> {
    let mut data = Vec::with_capacity(2 * PADDED * PADDED);
    for row in planes.stones {
        data.extend(row);
    }
    for row in planes.blocked {
        data.extend(row);
    }
    Tensor::<B, 4>::from_data(TensorData::new(data, [1, 2, PADDED, PADDED]), device)
}

fn maps_from_output<B: Backend>(
    b: &Board,
    s: Color,
    l1: Tensor<B, 4>,
    l2: Tensor<B, 4>,
    batch: usize,
) -> ThreatMaps {
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

    let l2_base = batch * LAYER2_OUT * 15 * 15;
    for ch in 0..LAYER2_OUT {
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

    let table = crate::kernels::pattern_table();

    for r in 0..15 {
        for c in 0..15 {
            let mv = engine::Move::new(r as u8, c as u8).expect("in-board indices");
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
                    let l1_idx = ((batch * LAYER1_OUT + ch) * 15 + r) * 15 + c;
                    if l1_data[l1_idx] > 0.0 {
                        for &(dy, dx) in &entry.completing_offsets {
                            let c2_r = r as i32 + dy;
                            let c2_c = c as i32 + dx;
                            // Occupied completing cells cannot exist,
                            // but guard anyway.
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
        Direction::DiagDown => dy, // dx == dy for points on this line
        Direction::DiagUp => dx,   // dx == -dy for points on this line
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use burn::backend::NdArray;
    use engine::reference::board_from_ascii;
    use engine::{Color, Move, MoveSet};

    fn set_of(cells: &[(u8, u8)]) -> MoveSet {
        let mut s = MoveSet::EMPTY;
        for &(r, c) in cells {
            s.insert(Move::new(r, c).unwrap());
        }
        s
    }

    #[test]
    fn open_four_win_map_matches_engine() {
        let b = board_from_ascii(
            "
            O . . . . . . . . . . . . . O
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
            . . . . . . . . . . . . . . .
            O . . . . . . . . . . . . . O
            ",
        );
        let device = Default::default();
        let net = ThreatNet::<NdArray>::new(&device);
        let maps = analyze(&net, &b, Color::Black, &device);
        assert_eq!(
            wins_set(&maps),
            set_of(&[(7, 3), (7, 8)]),
            "open four should have two winning cells"
        );
    }

    #[test]
    fn broken_four_win_map_matches_engine() {
        let b = board_from_ascii(
            "
            O . . . . . . . . . . . . . O
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . X X . X X . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            O . . . . . . . . . . . . . O
            ",
        );
        let device = Default::default();
        let net = ThreatNet::<NdArray>::new(&device);
        let maps = analyze(&net, &b, Color::Black, &device);
        assert_eq!(
            wins_set(&maps),
            set_of(&[(7, 6)]),
            "broken four should have one winning cell"
        );
    }

    #[test]
    fn classic_fork_double_threat() {
        // Two closed threes through (7,7); playing there opens two
        // fours, each with one winning cell.
        let b = board_from_ascii(
            "
            . . . . . . . . . . . . . . O
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . O . . . . . . .
            . . . . . . . X . . . . . . .
            . . . . . . . X . . . . . . .
            . . . . . . . X . . . . . . .
            . . . O X X X . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . O . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            O . . . . . . . . . . . . . O
            ",
        );
        let device = Default::default();
        let net = ThreatNet::<NdArray>::new(&device);
        let maps = analyze(&net, &b, Color::Black, &device);
        assert!(wins_set(&maps).is_empty(), "no immediate win");
        assert_eq!(
            double_threat_set(&maps),
            set_of(&[(7, 7)]),
            "classic fork is a single double threat"
        );
    }

    #[test]
    fn open_four_maker_is_double_threat() {
        // Three black stones with both ends empty. Extending either
        // end creates an open four, which is a double threat.
        let b = board_from_ascii(
            "
            O . . . . . . . . . . . . . O
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
            . . . . . . . . . . . . . . .
            O . . . . . . . . . . . . . .
            ",
        );
        let device = Default::default();
        let net = ThreatNet::<NdArray>::new(&device);
        let maps = analyze(&net, &b, Color::Black, &device);
        assert!(
            wins_set(&maps).is_empty(),
            "three stones alone do not complete five"
        );
        assert_eq!(
            double_threat_set(&maps),
            set_of(&[(7, 4), (7, 8)]),
            "both extensions create an open four (two winning cells)"
        );
    }

    #[test]
    fn two_broken_fours_make_double_threat() {
        // Black stones with two gaps; filling the middle gap creates
        // two broken fours whose two unique completing cells make a
        // double threat.
        let b = board_from_ascii(
            "
            O . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            X . X . X . X . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            . . . . . . . . . . . . . . .
            O . . . . . . . . . . . . . O
            ",
        );
        let device = Default::default();
        let net = ThreatNet::<NdArray>::new(&device);
        let maps = analyze(&net, &b, Color::Black, &device);
        assert_eq!(
            double_threat_set(&maps),
            set_of(&[(7, 3)]),
            "filling the middle gap creates two broken fours"
        );
    }

    #[test]
    fn double_three_fork_matches_oracle() {
        // Horizontal and vertical threes through (7,7).
        let b = board_from_ascii(
            "
            O . . . . . . . . . . . . . O
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
            . . . . . . . . . . . . . . .
            O . . . . . . . . . . . . . O
            ",
        );
        let device = Default::default();
        let net = ThreatNet::<NdArray>::new(&device);
        let maps = analyze(&net, &b, Color::Black, &device);
        assert_eq!(
            double_three_set(&maps),
            set_of(&[(7, 7)]),
            "only the intersection is a double-three fork"
        );
    }

    fn wins_set(maps: &ThreatMaps) -> MoveSet {
        set_from_map(&maps.wins)
    }

    fn double_threat_set(maps: &ThreatMaps) -> MoveSet {
        set_from_map(&maps.double_threats)
    }

    fn double_three_set(maps: &ThreatMaps) -> MoveSet {
        set_from_map(&maps.double_threes)
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
}
````
