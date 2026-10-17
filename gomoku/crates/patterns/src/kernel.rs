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

use crate::tools::conv_from_template;
use burn::module::Param;
use burn::nn::PaddingConfig2d;
use burn::nn::conv::{Conv2d, Conv2dConfig};
use burn::tensor::{Device, Tensor, TensorData};

/// Side length of the uniform square kernel footprint
pub const KERNEL: usize = 11;

/// Radius of the kernel footprint
pub const RADIUS: i32 = 5;

/// The four line directions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    Horizontal,
    Vertical,
    DiagDown,
    DiagUp,
}

impl Direction {
    /// all four directions
    pub const ALL: [Direction; 4] = [
        Direction::Horizontal,
        Direction::Vertical,
        Direction::DiagDown,
        Direction::DiagUp,
    ];

    /// the index of each direction - to locate the pattern
    pub fn index(self) -> usize {
        match self {
            Direction::Horizontal => 0,
            Direction::Vertical => 1,
            Direction::DiagDown => 2,
            Direction::DiagUp => 3,
        }
    }

    /// step vector (drow, dcol)
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
    /// xxxxc or alike
    FiveCompleter,
    /// cxxxc or alike
    OpenFourMaker,
    /// xcxcx or alike
    BrokenFourMaker,
    /// cxxc
    ThreeXxx,
    /// xxccx
    ThreeXxX,
    /// xccxx
    ThreeXxXRev,
}

impl PatternFamily {
    pub fn weight_scale(self) -> f32 {
        match self {
            PatternFamily::BrokenFourMaker => 1.0,
            PatternFamily::OpenFourMaker => 2.0,
            _ => 1.0,
        }
    }

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

    pub const fn num_channels(self) -> usize {
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
    pub family: PatternFamily,
    pub direction: Direction,
    pub stone_offsets: Vec<(i32, i32)>,
    pub empty_offsets: Vec<(i32, i32)>,
    pub anti_margin_offsets: Vec<(i32, i32)>,
    pub completing_offsets: Vec<(i32, i32)>,
}

/// Total number of output channels produced by layer 1.
pub const LAYER1_NUM_CHANNELS: usize = PatternFamily::FiveCompleter.num_channels()
    + PatternFamily::OpenFourMaker.num_channels()
    + PatternFamily::BrokenFourMaker.num_channels()
    + PatternFamily::ThreeXxx.num_channels()
    + PatternFamily::ThreeXxX.num_channels()
    + PatternFamily::ThreeXxXRev.num_channels();

// Number of output channels produced by layer 2.
///
/// Layer 2 emits: one win map, four per-direction four-created maps,
/// and four per-direction three maps. The final double-threat and
/// double-three maps are derived from these by the small amount of
/// glue code that chapter 4 adds to `net.rs` as `ThreatMaps` (the
/// `before` scalar and win masking are board-dependent, so they live
/// outside the conv).
pub const LAYER2_NUM_CHANNELS: usize = 9;

/// Build the complete pattern table.
///
/// The table contains every layer-1 kernel shape in channel order:
/// for each direction in canonical order, all shapes of
/// `FiveCompleter`, then `OpenFourMaker`, then `BrokenFourMaker`,
/// then `ThreeXxx`, `ThreeXxX`, and `ThreeXxXRev`.
pub fn pattern_table() -> Vec<PatternEntry> {
    let mut table = Vec::with_capacity(LAYER1_NUM_CHANNELS);

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

    assert_eq!(table.len(), LAYER1_NUM_CHANNELS);
    table
}

/// Build layer 1: an 11×11 conv from 2 input channels to
/// [`LAYER1_NUM_CHANNELS`] output channels.
///
/// The returned conv uses valid padding and a ReLU activation (the
/// caller supplies the activation by calling `forward` in the normal
/// Burn way).
pub fn layer1_conv(device: &Device) -> Conv2d {
    let table = pattern_table();

    let template: Conv2d = Conv2dConfig::new([2, LAYER1_NUM_CHANNELS], [KERNEL, KERNEL])
        .with_padding(PaddingConfig2d::Valid)
        .with_bias(true)
        .init(device);

    let mut weight = vec![0.0f32; LAYER1_NUM_CHANNELS * 2 * KERNEL * KERNEL];
    let mut bias = vec![0.0f32; LAYER1_NUM_CHANNELS];

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

    let weight_tensor = Tensor::<4>::from_data(
        TensorData::new(weight, [LAYER1_NUM_CHANNELS, 2, KERNEL, KERNEL]),
        device,
    );

    let bias_tensor = Tensor::<1>::from_data(TensorData::new(bias, [LAYER1_NUM_CHANNELS]), device);

    conv_from_template(weight_tensor, bias_tensor, template)
}

/// Build layer 2: a 1×1 conv from [`LAYER1_NUM_CHANNELS`] channels to
/// [`LAYER2_NUM_CHANNELS`] channels.
///
/// Output channel layout:
///
/// * `0` — win map (OR of all five-completer channels).
/// * `1..=4` — five-completing cells created in each direction
///   (`Horizontal`, `Vertical`, `DiagDown`, `DiagUp`).
/// * `5..=8` — open-three makers in each direction.
pub fn layer2_conv(device: &Device) -> Conv2d {
    let table = pattern_table();

    let template: Conv2d = Conv2dConfig::new([LAYER1_NUM_CHANNELS, LAYER2_NUM_CHANNELS], [1, 1])
        .with_padding(PaddingConfig2d::Valid)
        .with_bias(true)
        .init(device);

    let mut weight = vec![0.0f32; LAYER1_NUM_CHANNELS * LAYER2_NUM_CHANNELS];
    let bias = vec![0.0f32; LAYER2_NUM_CHANNELS];

    for (in_ch, entry) in table.iter().enumerate() {
        // channel 0: wins:
        if matches!(entry.family, PatternFamily::FiveCompleter) {
            set_weight_1x1(&mut weight, 0, in_ch, 1.0);
        }

        // Channels 1..=4: fours created, per direction.
        match entry.family {
            PatternFamily::OpenFourMaker | PatternFamily::BrokenFourMaker => {
                set_weight_1x1(
                    &mut weight,
                    1 + entry.direction.index(),
                    in_ch,
                    entry.family.weight_scale(),
                );
            }
            _ => {}
        }

        // Channels 5..=8: threes, per direction.
        let three_idx = 5 + entry.direction.index();
        match entry.family {
            PatternFamily::ThreeXxX | PatternFamily::ThreeXxx | PatternFamily::ThreeXxXRev => {
                set_weight_1x1(&mut weight, three_idx, in_ch, 1.0);
            }
            _ => {}
        }
    }

    let weight_tensor = Tensor::<4>::from_data(
        TensorData::new(weight, [LAYER2_NUM_CHANNELS, LAYER1_NUM_CHANNELS, 1, 1]),
        device,
    );
    let bias_tensor = Tensor::<1>::from_data(TensorData::new(bias, [LAYER2_NUM_CHANNELS]), device);

    Conv2d {
        weight: Param::from_tensor(weight_tensor),
        bias: Some(Param::from_tensor(bias_tensor)),
        kernel_size: template.kernel_size,
        stride: template.stride,
        dilation: template.dilation,
        groups: template.groups,
        padding: template.padding,
    }
}

// ------------------------------------------------------------------
// Base shape generators (horizontal direction, `c` at the origin).
// Offsets are 1-D positions along the line; `c` is position 0.
// ------------------------------------------------------------------
fn five_completer_base_shapes() -> Vec<PatternEntry> {
    let mut out = Vec::with_capacity(PatternFamily::FiveCompleter.base_shape_count());

    for k in 0..5i32 {
        let window: Vec<i32> = (-k..=(-k + 4)).collect();
        let stones: Vec<(i32, i32)> = as_coords_without_origin(window);

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

fn as_coords_without_origin(steps: Vec<i32>) -> Vec<(i32, i32)> {
    steps.iter().filter(|&&p| p != 0).map(|&p| (0, p)).collect()
}

fn open_four_maker_base_shapes() -> Vec<PatternEntry> {
    let mut out = Vec::with_capacity(PatternFamily::OpenFourMaker.base_shape_count());
    for k in 0..4i32 {
        let segment: Vec<i32> = (-k..=(-k + 3)).collect();
        let stones = as_coords_without_origin(segment);

        let empty = vec![(0, -k - 1), (0, -k + 4)];
        let completing = vec![(0, -k - 1), (0, -k + 4)];

        out.push(PatternEntry {
            family: PatternFamily::OpenFourMaker,
            direction: Direction::Horizontal,
            stone_offsets: stones,
            empty_offsets: empty,
            anti_margin_offsets: Vec::new(),
            completing_offsets: completing,
        })
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
    for k in 1..=3i32 {
        out.push(make_three_shape(
            PatternFamily::ThreeXxx,
            k,
            &[1, 2, 3],
            &[0, 4],
        ))
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
    let shift = -c_pos;
    let left_most = shift;
    let right_most = shift + empty_positions.iter().max().copied().unwrap_or(0);

    let stones: Vec<(i32, i32)> = stone_positions
        .iter()
        .map(|&p| (0, shift + p))
        .filter(|&(_, p)| p != 0)
        .collect();
    let empty: Vec<(i32, i32)> = empty_positions.iter().map(|&p| (0, shift + p)).collect();
    let anti_margin = vec![(0, left_most - 1), (0, right_most + 1)];

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
// Rotation and tensor helpers
// ------------------------------------------------------------------

fn rotate(entry: &PatternEntry, dir: Direction) -> PatternEntry {
    let (dr, dc) = dir.step();
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
        "offset ({dy}, {dx}) exceeds kernel radius {RADIUS} "
    );
    let ky = (dy + RADIUS) as usize;
    let kx = (dx + RADIUS) as usize;
    let idx = ((out_ch * 2 + in_ch) * KERNEL + ky) * KERNEL + kx;

    weight[idx] = value;
}

fn set_weight_1x1(weight: &mut [f32], out_ch: usize, in_ch: usize, value: f32) {
    let idx = out_ch * LAYER1_NUM_CHANNELS + in_ch;
    weight[idx] = value;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_counts_match_table() {
        let table = pattern_table();
        assert_eq!(table.len(), LAYER1_NUM_CHANNELS);
        assert_eq!(LAYER1_NUM_CHANNELS, 152);
        assert_eq!(LAYER2_NUM_CHANNELS, 9);
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
                    "{entry:?} contains offset ({dy}, {dx} outside radius {RADIUS}"
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
                family.num_channels(),
                "family {family:?} channel count mismatch"
            );
        }
    }

    #[test]
    fn display_patterns() {
        let device = Device::flex();

        let layer1 = layer1_conv(&device);

        let data = layer1.weight.to_data();
        let weights: &[f32] = data.as_slice().unwrap();

        let (p11, _) = weights.as_chunks::<121>();
        let (p11_2, _) = p11.as_chunks::<2>();

        let p = p11_2[0][0];
        let (rows, _) = p.as_chunks::<11>();

        rows.iter().for_each(|row| println!("{:?}", row));

        // for all plane pairs of all patterns
        for p121_2 in weights.as_chunks::<121>().0.as_chunks::<2>().0 {
            // for both planes of the pair
            for planes in p121_2 {
                let rows = planes.as_chunks::<11>().0;
                for row in rows {
                    let row_image = row
                        .iter()
                        .map(|v| match v {
                            0.0 => " . ",
                            1.0 => " x ",
                            -1.0 => " # ",
                            _ => " ? ",
                        })
                        .collect::<String>();
                    println!("{}", row_image);
                }
                println!();
            }
            println!();
        }
    }
}
