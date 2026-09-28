> **Opt-in solution — read only after you have tried the chapter, or if you have been stuck for more than twenty minutes.**
>
> This file quotes `src/kernels.rs` from the verified reference crate in
> its **final** form. The families beyond `FiveCompleter` are added in
> chapters 4 and 5; the chapter-3 learner implements only
> `FiveCompleter` (and the shared machinery), leaving the remaining
> families as placeholders. Quoting the final file here lets you compare
> the shape of the finished artifact without spoiling the incremental
> build.

````rust
//! Declarative pattern table and hand-manufactured convolution kernels.
//!
//! This module is the kernel factory for the threat network. It defines
//! every local pattern from the design spec, §3, as a small table entry:
//! the cells that must hold own stones, the cells that must be empty,
//! and (for open-three patterns) the cells that must *not* hold an own
//! stone. The table is then expanded into 11×11 convolution kernels for
//! every board direction.
//!
//! The kernel language is a direct encoding of exact Boolean conditions:
//!
//! * `+1` on a required-stone cell in channel 0 (own stones).
//! * `−1` on a required-empty cell in *both* channels — a stone of
//!   either colour, or the materialised border, violates emptiness.
//! * `−1` on an anti-four margin cell in channel 0 only — the cell may
//!   be empty or occupied by the opponent, but an own stone turns the
//!   pattern into a four or five.
//! * bias = `1 − number_of_required_stones`, so the pre-activation is
//!   exactly `1` on a perfect match and drops by `1` for every violated
//!   condition. ReLU therefore emits a clean `1` for a match and `0`
//!   otherwise.
//!
//! All kernel weights are small integers, so every f32 sum is exact on
//! every Burn backend.

use burn::module::Param;
use burn::nn::PaddingConfig2d;
use burn::nn::conv::{Conv2d, Conv2dConfig};
use burn::tensor::backend::Backend;
use burn::tensor::{Tensor, TensorData};

/// Side length of the uniform square kernel footprint.
pub const KERNEL: usize = 11;

/// Radius of the kernel footprint: `(KERNEL - 1) / 2`.
pub const RADIUS: i32 = 5;

/// The four line directions, in the same order as the engine's win
/// detector and the oracle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    /// Left-to-right.
    Horizontal,
    /// Top-to-bottom.
    Vertical,
    /// Top-left to bottom-right.
    DiagDown,
    /// Bottom-left to top-right.
    DiagUp,
}

impl Direction {
    /// All four directions in canonical order.
    pub const ALL: [Direction; 4] = [
        Direction::Horizontal,
        Direction::Vertical,
        Direction::DiagDown,
        Direction::DiagUp,
    ];

    /// Direction index in `0..4`.
    pub fn index(self) -> usize {
        match self {
            Direction::Horizontal => 0,
            Direction::Vertical => 1,
            Direction::DiagDown => 2,
            Direction::DiagUp => 3,
        }
    }

    /// Step vector `(drow, dcol)` for walking one cell along this
    /// direction.
    pub fn step(self) -> (i32, i32) {
        match self {
            Direction::Horizontal => (0, 1),
            Direction::Vertical => (1, 0),
            Direction::DiagDown => (1, 1),
            Direction::DiagUp => (-1, 1),
        }
    }
}

/// Pattern family. Each family groups shapes that detect the same kind
/// of threat and share a layer-2 weight scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PatternFamily {
    /// Five-cell window containing `c`; placing at `c` completes five.
    FiveCompleter,
    /// Four consecutive stones including `c` with both ends empty.
    OpenFourMaker,
    /// Four stones in a five-cell window with one gap; the gap is the
    /// unique five-completing cell.
    BrokenFourMaker,
    /// `_XXX_` open three.
    ThreeXxx,
    /// `_X_XX_` open three.
    ThreeXxX,
    /// `_XX_X_` open three.
    ThreeXxXRev,
}

impl PatternFamily {
    /// Scale applied in layer 2 so that the channel contributes the
    /// correct number of five-completing cells.
    pub fn weight_scale(self) -> f32 {
        match self {
            // One five-completing cell per broken four.
            PatternFamily::BrokenFourMaker => 1.0,
            // One open four contributes two five-completing cells.
            PatternFamily::OpenFourMaker => 2.0,
            // Threes and immediate wins are counted direction-wise in
            // layer 2; their raw scale is 1.
            _ => 1.0,
        }
    }

    /// Number of distinct horizontal base shapes for this family.
    ///
    /// A base shape fixes `c` at the origin and describes the pattern
    /// along the horizontal direction. Direction variants are generated
    /// by rotation.
    pub const fn base_shape_count(self) -> usize {
        match self {
            PatternFamily::FiveCompleter => 5,
            PatternFamily::OpenFourMaker => 4,
            PatternFamily::BrokenFourMaker => 20,
            PatternFamily::ThreeXxx => 3,
            PatternFamily::ThreeXxX => 3,
            PatternFamily::ThreeXxXRev => 3,
        }
    }

    /// Total channels contributed by this family across all directions.
    pub const fn channels(self) -> usize {
        self.base_shape_count() * 4
    }
}

/// One declarative pattern entry.
///
/// Offsets are in padded-board coordinates relative to the candidate
/// cell `c`. The kernel centre corresponds to `c`; an offset `(dy,dx)`
/// is stored at kernel position `(RADIUS + dy, RADIUS + dx)`.
#[derive(Debug, Clone)]
pub struct PatternEntry {
    /// Which family this shape belongs to.
    pub family: PatternFamily,
    /// Which direction this kernel detects.
    pub direction: Direction,
    /// Cells that must hold an own stone of the evaluated side.
    pub stone_offsets: Vec<(i32, i32)>,
    /// Cells that must be genuinely empty (no stone of either colour,
    /// and not the materialised border).
    pub empty_offsets: Vec<(i32, i32)>,
    /// Cells that must *not* hold an own stone. They may be empty or
    /// occupied by the opponent. Used only for the anti-four margins
    /// of open-three patterns.
    pub anti_margin_offsets: Vec<(i32, i32)>,
    /// The five-completing cells produced by this pattern, relative to
    /// `c`. An open four contributes two cells (its ends); a broken
    /// four contributes one (its gap). A five-completer contributes
    /// `c` itself.
    pub completing_offsets: Vec<(i32, i32)>,
}

/// Total number of output channels produced by layer 1.
pub const LAYER1_OUT: usize = PatternFamily::FiveCompleter.channels()
    + PatternFamily::OpenFourMaker.channels()
    + PatternFamily::BrokenFourMaker.channels()
    + PatternFamily::ThreeXxx.channels()
    + PatternFamily::ThreeXxX.channels()
    + PatternFamily::ThreeXxXRev.channels();

/// Number of output channels produced by layer 2.
///
/// Layer 2 emits: one win map, four per-direction four-created maps,
/// and four per-direction three maps. The final double-threat and
/// double-three maps are derived from these by the small amount of
/// glue code in [`crate::net::ThreatMaps`] (the `before` scalar and
/// win masking are board-dependent, so they live outside the conv).
pub const LAYER2_OUT: usize = 9;

/// Build the complete pattern table.
///
/// The table contains every layer-1 kernel shape in channel order:
/// for each direction in canonical order, all shapes of
/// `FiveCompleter`, then `OpenFourMaker`, then `BrokenFourMaker`,
/// then `ThreeXxx`, `ThreeXxX`, and `ThreeXxXRev`.
pub fn pattern_table() -> Vec<PatternEntry> {
    let mut table = Vec::with_capacity(LAYER1_OUT);

    for dir in Direction::ALL {
        for base in five_completer_base_shapes() {
            table.push(rotate(&base, dir));
        }
        for base in open_four_maker_base_shapes() {
            table.push(rotate(&base, dir));
        }
        for base in broken_four_maker_base_shapes() {
            table.push(rotate(&base, dir));
        }
        for base in three_xxx_base_shapes() {
            table.push(rotate(&base, dir));
        }
        for base in three_x_xx_base_shapes() {
            table.push(rotate(&base, dir));
        }
        for base in three_xx_x_base_shapes() {
            table.push(rotate(&base, dir));
        }
    }

    assert_eq!(table.len(), LAYER1_OUT);
    table
}

/// Build layer 1: an 11×11 conv from 2 input channels to
/// [`LAYER1_OUT`] output channels.
///
/// The returned conv uses valid padding and a ReLU activation (the
/// caller supplies the activation by calling `forward` in the normal
/// Burn way).
pub fn layer1_conv<B: Backend>(device: &B::Device) -> Conv2d<B> {
    let table = pattern_table();

    let conv: Conv2d<B> = Conv2dConfig::new([2, LAYER1_OUT], [KERNEL, KERNEL])
        .with_padding(PaddingConfig2d::Valid)
        .with_bias(true)
        .init(device);

    // Burn 0.21 has no weight setter, so init-from-config gives us the
    // metadata; `weight`/`bias` are then replaced via `Param::from_tensor`.
    // See SPIKE.md.
    let mut weight = vec![0.0f32; LAYER1_OUT * 2 * KERNEL * KERNEL];
    let mut bias = vec![0.0f32; LAYER1_OUT];

    for (out_ch, entry) in table.iter().enumerate() {
        let num_stones = entry.stone_offsets.len();
        bias[out_ch] = 1.0 - num_stones as f32;

        for &(dy, dx) in &entry.stone_offsets {
            set_weight(&mut weight, out_ch, 0, dy, dx, 1.0);
        }
        for &(dy, dx) in &entry.empty_offsets {
            set_weight(&mut weight, out_ch, 0, dy, dx, -1.0);
            set_weight(&mut weight, out_ch, 1, dy, dx, -1.0);
        }
        for &(dy, dx) in &entry.anti_margin_offsets {
            set_weight(&mut weight, out_ch, 0, dy, dx, -1.0);
        }
    }

    let weight_tensor = Tensor::<B, 4>::from_data(
        TensorData::new(weight, [LAYER1_OUT, 2, KERNEL, KERNEL]),
        device,
    );
    let bias_tensor = Tensor::<B, 1>::from_data(TensorData::new(bias, [LAYER1_OUT]), device);

    Conv2d {
        weight: Param::from_tensor(weight_tensor),
        bias: Some(Param::from_tensor(bias_tensor)),
        stride: conv.stride,
        kernel_size: conv.kernel_size,
        dilation: conv.dilation,
        groups: conv.groups,
        padding: conv.padding,
    }
}

/// Build layer 2: a 1×1 conv from [`LAYER1_OUT`] channels to
/// [`LAYER2_OUT`] channels.
///
/// Output channel layout:
///
/// * `0` — win map (OR of all five-completer channels).
/// * `1..=4` — five-completing cells created in each direction
///   (`Horizontal`, `Vertical`, `DiagDown`, `DiagUp`).
/// * `5..=8` — open-three makers in each direction.
pub fn layer2_conv<B: Backend>(device: &B::Device) -> Conv2d<B> {
    let table = pattern_table();

    let conv: Conv2d<B> = Conv2dConfig::new([LAYER1_OUT, LAYER2_OUT], [1, 1])
        .with_padding(PaddingConfig2d::Valid)
        .with_bias(true)
        .init(device);

    // Burn 0.21 has no weight setter, so init-from-config gives us the
    // metadata; `weight`/`bias` are then replaced via `Param::from_tensor`.
    // See SPIKE.md.
    let mut weight = vec![0.0f32; LAYER2_OUT * LAYER1_OUT];
    let bias = vec![0.0f32; LAYER2_OUT];

    for (in_ch, entry) in table.iter().enumerate() {
        // Channel 0: wins.
        if matches!(entry.family, PatternFamily::FiveCompleter) {
            set_weight_1x1(&mut weight, 0, in_ch, 1.0);
        }

        // Channels 1..=4: fours created, per direction.
        let dir_idx = 1 + entry.direction.index();
        match entry.family {
            PatternFamily::OpenFourMaker | PatternFamily::BrokenFourMaker => {
                set_weight_1x1(&mut weight, dir_idx, in_ch, entry.family.weight_scale());
            }
            _ => {}
        }

        // Channels 5..=8: threes, per direction.
        let three_idx = 5 + entry.direction.index();
        match entry.family {
            PatternFamily::ThreeXxx | PatternFamily::ThreeXxX | PatternFamily::ThreeXxXRev => {
                set_weight_1x1(&mut weight, three_idx, in_ch, 1.0);
            }
            _ => {}
        }
    }

    let weight_tensor = Tensor::<B, 4>::from_data(
        TensorData::new(weight, [LAYER2_OUT, LAYER1_OUT, 1, 1]),
        device,
    );
    let bias_tensor = Tensor::<B, 1>::from_data(TensorData::new(bias, [LAYER2_OUT]), device);

    Conv2d {
        weight: Param::from_tensor(weight_tensor),
        bias: Some(Param::from_tensor(bias_tensor)),
        stride: conv.stride,
        kernel_size: conv.kernel_size,
        dilation: conv.dilation,
        groups: conv.groups,
        padding: conv.padding,
    }
}

// ------------------------------------------------------------------
// Base shape generators (horizontal direction, `c` at the origin).
// Offsets are 1-D positions along the line; `c` is position 0.
// ------------------------------------------------------------------

fn five_completer_base_shapes() -> Vec<PatternEntry> {
    let mut out = Vec::with_capacity(PatternFamily::FiveCompleter.base_shape_count());
    // `c` sits at offset `k` inside a 5-cell window [-k, -k+1, ..., -k+4].
    for k in 0..5i32 {
        let window: Vec<i32> = (-k..=(-k + 4)).collect();
        let stones: Vec<(i32, i32)> = window
            .iter()
            .filter(|&&p| p != 0)
            .map(|&p| (0, p))
            .collect();
        out.push(PatternEntry {
            family: PatternFamily::FiveCompleter,
            direction: Direction::Horizontal,
            stone_offsets: stones,
            empty_offsets: Vec::new(),
            anti_margin_offsets: Vec::new(),
            completing_offsets: vec![(0, 0)],
        });
    }
    out
}

fn open_four_maker_base_shapes() -> Vec<PatternEntry> {
    let mut out = Vec::with_capacity(PatternFamily::OpenFourMaker.base_shape_count());
    // `c` sits at offset `k` inside a 4-cell segment [-k, ..., -k+3].
    // The two cells immediately outside the segment must be empty.
    for k in 0..4i32 {
        let segment: Vec<i32> = (-k..=(-k + 3)).collect();
        let stones: Vec<(i32, i32)> = segment
            .iter()
            .filter(|&&p| p != 0)
            .map(|&p| (0, p))
            .collect();
        let empty = vec![(0, -k - 1), (0, -k + 4)];
        // The two five-completing cells are the segment ends.
        let completing = vec![(0, -k - 1), (0, -k + 4)];
        out.push(PatternEntry {
            family: PatternFamily::OpenFourMaker,
            direction: Direction::Horizontal,
            stone_offsets: stones,
            empty_offsets: empty,
            anti_margin_offsets: Vec::new(),
            completing_offsets: completing,
        });
    }
    out
}

fn broken_four_maker_base_shapes() -> Vec<PatternEntry> {
    let mut out = Vec::with_capacity(PatternFamily::BrokenFourMaker.base_shape_count());
    // `c` sits at offset `k` inside a 5-cell window [-k, ..., -k+4].
    // One of the other four cells is the gap (empty); the remaining
    // three are stones.
    for k in 0..5i32 {
        let window: Vec<i32> = (-k..=(-k + 4)).collect();
        for gap in window.iter().copied().filter(|&p| p != 0) {
            let stones: Vec<(i32, i32)> = window
                .iter()
                .filter(|&&p| p != 0 && p != gap)
                .map(|&p| (0, p))
                .collect();
            let empty = vec![(0, gap)];
            out.push(PatternEntry {
                family: PatternFamily::BrokenFourMaker,
                direction: Direction::Horizontal,
                stone_offsets: stones,
                empty_offsets: empty,
                anti_margin_offsets: Vec::new(),
                completing_offsets: vec![(0, gap)],
            });
        }
    }
    out
}

fn three_xxx_base_shapes() -> Vec<PatternEntry> {
    let mut out = Vec::with_capacity(PatternFamily::ThreeXxx.base_shape_count());
    // Pattern `_XXX_`: stones at pattern positions 1,2,3; empty at 0,4.
    // `c` is one of the three stones.
    for k in 1..=3i32 {
        out.push(make_three_shape(
            PatternFamily::ThreeXxx,
            k,
            &[1, 2, 3],
            &[0, 4],
        ));
    }
    out
}

fn three_x_xx_base_shapes() -> Vec<PatternEntry> {
    let mut out = Vec::with_capacity(PatternFamily::ThreeXxX.base_shape_count());
    // Pattern `_X_XX_`: stones at 1,3,4; empty at 0,2,5.
    for k in [1, 3, 4] {
        out.push(make_three_shape(
            PatternFamily::ThreeXxX,
            k,
            &[1, 3, 4],
            &[0, 2, 5],
        ));
    }
    out
}

fn three_xx_x_base_shapes() -> Vec<PatternEntry> {
    let mut out = Vec::with_capacity(PatternFamily::ThreeXxXRev.base_shape_count());
    // Pattern `_XX_X_`: stones at 1,2,4; empty at 0,3,5.
    for k in [1, 2, 4] {
        out.push(make_three_shape(
            PatternFamily::ThreeXxXRev,
            k,
            &[1, 2, 4],
            &[0, 3, 5],
        ));
    }
    out
}

fn make_three_shape(
    family: PatternFamily,
    c_pos: i32,
    stone_positions: &[i32],
    empty_positions: &[i32],
) -> PatternEntry {
    // The pattern window starts at offset `-c_pos` relative to `c`.
    let shift = -c_pos;
    let leftmost = shift;
    let rightmost = shift + empty_positions.iter().max().copied().unwrap_or(0);

    let stones: Vec<(i32, i32)> = stone_positions
        .iter()
        .map(|&p| (0, shift + p))
        .filter(|&(_, p)| p != 0)
        .collect();
    let empty: Vec<(i32, i32)> = empty_positions.iter().map(|&p| (0, shift + p)).collect();
    let anti_margin = vec![(0, leftmost - 1), (0, rightmost + 1)];

    PatternEntry {
        family,
        direction: Direction::Horizontal,
        stone_offsets: stones,
        empty_offsets: empty,
        anti_margin_offsets: anti_margin,
        completing_offsets: Vec::new(),
    }
}

// ------------------------------------------------------------------
// Rotation and tensor helpers.
// ------------------------------------------------------------------

fn rotate(entry: &PatternEntry, dir: Direction) -> PatternEntry {
    let (dr, dc) = dir.step();
    // The base shape is horizontal: offsets are `(0, p)` where `p`
    // increases to the right. Rotating by `(dr,dc)` maps the horizontal
    // vector `(0,1)` to `(dr,dc)`, so `(0,p)` becomes `(dr*p, dc*p)`.
    let rot_simple = |(_dy, dx): &(i32, i32)| (dr * dx, dc * dx);

    PatternEntry {
        family: entry.family,
        direction: dir,
        stone_offsets: entry.stone_offsets.iter().map(rot_simple).collect(),
        empty_offsets: entry.empty_offsets.iter().map(rot_simple).collect(),
        anti_margin_offsets: entry.anti_margin_offsets.iter().map(rot_simple).collect(),
        completing_offsets: entry.completing_offsets.iter().map(rot_simple).collect(),
    }
}

fn set_weight(weight: &mut [f32], out_ch: usize, in_ch: usize, dy: i32, dx: i32, value: f32) {
    assert!(
        dy.abs() <= RADIUS && dx.abs() <= RADIUS,
        "offset ({dy},{dx}) exceeds kernel radius {RADIUS}"
    );
    let ky = (dy + RADIUS) as usize;
    let kx = (dx + RADIUS) as usize;
    let idx = ((out_ch * 2 + in_ch) * KERNEL + ky) * KERNEL + kx;
    weight[idx] = value;
}

fn set_weight_1x1(weight: &mut [f32], out_ch: usize, in_ch: usize, value: f32) {
    let idx = out_ch * LAYER1_OUT + in_ch;
    weight[idx] = value;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_counts_match_table() {
        let table = pattern_table();
        assert_eq!(table.len(), LAYER1_OUT);
        assert_eq!(LAYER1_OUT, 152);
        assert_eq!(LAYER2_OUT, 9);
    }

    #[test]
    fn every_offset_fits_in_11x11_kernel() {
        for entry in pattern_table() {
            for &(dy, dx) in entry
                .stone_offsets
                .iter()
                .chain(&entry.empty_offsets)
                .chain(&entry.anti_margin_offsets)
            {
                assert!(
                    dy.abs() <= RADIUS && dx.abs() <= RADIUS,
                    "{entry:?} contains offset ({dy},{dx}) outside radius {RADIUS}"
                );
            }
        }
    }

    #[test]
    fn family_counts_are_correct() {
        let table = pattern_table();
        for family in [
            PatternFamily::FiveCompleter,
            PatternFamily::OpenFourMaker,
            PatternFamily::BrokenFourMaker,
            PatternFamily::ThreeXxx,
            PatternFamily::ThreeXxX,
            PatternFamily::ThreeXxXRev,
        ] {
            let count = table.iter().filter(|e| e.family == family).count();
            assert_eq!(
                count,
                family.channels(),
                "family {family:?} channel count mismatch"
            );
        }
    }
}
````
