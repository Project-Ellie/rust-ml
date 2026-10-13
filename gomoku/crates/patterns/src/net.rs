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

use crate::kernel::{layer1_conv, layer2_conv};
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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn open_four_win_map_matches_engine() {
        // Black has an open four on row 7 (cols 4..=7): both ends,
        // (7, 3) and (7, 8), complete five immediately.
        let image = "\
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
        let image = "\
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
}
