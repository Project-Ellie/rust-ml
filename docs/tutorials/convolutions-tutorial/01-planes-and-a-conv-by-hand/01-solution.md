# Chapter 01 — Deep-dive solution: planes + hand-rolled cross-correlation

This is the opt-in reference for Chapter 01. Try the chapter for at least
twenty minutes before opening this file; then use it to compare or to
unblock yourself.

The files below were extracted verbatim from the verified reference crate
and byte-identical copies were confirmed with `diff -u` against the
reference worktree before this file was written.

## `src/planes.rs`

```rust
//! Input planes for the hand-written threat network.
//!
//! A Gomoku board is 15×15. The network uses a 25×25 materialised
//! padding of 5 cells on every side so that an 11×11 valid convolution
//! sees the board edge as blocked instead of zero-padded. The larger
//! radius is required by the broken-three patterns (`_X_XX_`,
//! `_XX_X_`), whose anti-four margin sits five cells away from the
//! candidate move. See the design spec, §4.1 and §4.2.

use engine::{Board, Color, Move};

/// Padding radius around the 15×15 board.
pub const PAD: usize = 5;

/// Padded plane side length: `15 + 2 * PAD`.
pub const PADDED: usize = 25;

/// Two-channel padded input for side `s`.
///
/// * `stones` — `1.0` on cells occupied by `s`, `0.0` elsewhere,
///   including the zero-padded border.
/// * `blocked` — `1.0` on opponent stones and on every border cell,
///   `0.0` on empty in-board cells.
///
/// The two channels are kept as plain fixed-size arrays so they can be
/// turned into Burn tensors in later tutorial tasks.
pub struct Planes {
    /// The side's own stones as a 25×25 `f32` array.
    pub stones: [[f32; PADDED]; PADDED],
    /// Blocked cells (opponent stones + border) as a 25×25 `f32` array.
    pub blocked: [[f32; PADDED]; PADDED],
}

/// Build the two padded input channels for side `s`.
///
/// The border is treated as blocked, not empty. This makes edge
/// behaviour fall out of the pattern kernels: a required-empty cell
/// that lies off the board is not empty, so a three hugging the edge
/// is correctly rejected as an open three.
pub fn planes(b: &Board, s: Color) -> Planes {
    let mut out = Planes {
        stones: [[0.0; PADDED]; PADDED],
        blocked: [[1.0; PADDED]; PADDED],
    };

    for r in 0..15u8 {
        for c in 0..15u8 {
            let pr = (r as usize) + PAD;
            let pc = (c as usize) + PAD;
            let mv = Move::new(r, c).expect("in-board indices are legal moves");
            match b.stone_at(mv) {
                Some(color) if color == s => {
                    out.stones[pr][pc] = 1.0;
                    out.blocked[pr][pc] = 0.0;
                }
                Some(_) => {
                    // opponent stone
                    out.stones[pr][pc] = 0.0;
                    out.blocked[pr][pc] = 1.0;
                }
                None => {
                    out.stones[pr][pc] = 0.0;
                    out.blocked[pr][pc] = 0.0;
                }
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{Board, Color, Move};

    /// Number of border cells in a 25×25 plane padded by 5.
    const BORDER_CELLS: usize = PADDED * PADDED - 15 * 15;

    fn count(v: f32, plane: &[[f32; PADDED]; PADDED]) -> usize {
        plane.iter().flatten().filter(|&&x| x == v).count()
    }

    #[test]
    fn empty_board_has_zero_stones_and_blocked_border_only() {
        let b = Board::new();
        let p = planes(&b, Color::Black);

        assert_eq!(count(1.0, &p.stones), 0, "no stones on an empty board");
        assert_eq!(count(0.0, &p.blocked), 15 * 15, "in-board cells are empty");
        assert_eq!(count(1.0, &p.blocked), BORDER_CELLS, "border is blocked");
    }

    #[test]
    fn stones_are_placed_for_the_requested_side() {
        let mut b = Board::new();
        b.play(Move::new(7, 7).unwrap()).unwrap(); // Black
        b.play(Move::new(6, 6).unwrap()).unwrap(); // White
        b.play(Move::new(8, 8).unwrap()).unwrap(); // Black

        let black = planes(&b, Color::Black);
        let white = planes(&b, Color::White);

        assert_eq!(black.stones[7 + PAD][7 + PAD], 1.0);
        assert_eq!(black.stones[8 + PAD][8 + PAD], 1.0);
        assert_eq!(
            black.stones[6 + PAD][6 + PAD],
            0.0,
            "opponent stone is not own"
        );

        assert_eq!(white.stones[6 + PAD][6 + PAD], 1.0);
        assert_eq!(white.stones[7 + PAD][7 + PAD], 0.0);

        // Black channel sees the white stone as blocked, not as a stone.
        assert_eq!(black.blocked[6 + PAD][6 + PAD], 1.0);
        assert_eq!(black.blocked[7 + PAD][7 + PAD], 0.0);
    }

    #[test]
    fn corner_and_edge_stones_are_mapped_correctly() {
        let mut b = Board::new();
        b.play(Move::new(0, 0).unwrap()).unwrap();
        b.play(Move::new(0, 14).unwrap()).unwrap();
        b.play(Move::new(14, 0).unwrap()).unwrap();
        b.play(Move::new(14, 14).unwrap()).unwrap();
        b.play(Move::new(7, 14).unwrap()).unwrap();

        let p = planes(&b, Color::Black);
        assert_eq!(p.stones[PAD][PAD], 1.0);
        assert_eq!(p.stones[14 + PAD][PAD], 1.0);
        assert_eq!(p.stones[7 + PAD][14 + PAD], 1.0);
        assert_eq!(p.stones[PAD][14 + PAD], 0.0, "white stone is not black");
        assert_eq!(p.blocked[PAD][14 + PAD], 1.0, "white stone is blocked");
    }

    #[test]
    fn border_ring_is_exactly_five_cells_wide() {
        let b = Board::new();
        let p = planes(&b, Color::Black);

        // Inside the 15×15 active region blocked is 0; outside it is 1.
        for (r, row) in p.blocked.iter().enumerate() {
            for (c, &v) in row.iter().enumerate() {
                let in_board = (PAD..PADDED - PAD).contains(&r) && (PAD..PADDED - PAD).contains(&c);
                if in_board {
                    assert_eq!(v, 0.0, "({r},{c}) must be empty");
                } else {
                    assert_eq!(v, 1.0, "({r},{c}) must be border");
                }
            }
        }
    }
}
```

## `src/naive.rs`

```rust
//! Learner's hand-rolled cross-correlation on plain Rust arrays.
//!
//! This module is the reference implementation for chapter 1 of the
//! convolutions tutorial. It performs a 2-D cross-correlation (what deep
//! learning calls a "convolution") on single-channel `f32` maps with valid
//! (no) padding.

/// Apply a 2-D cross-correlation with valid padding and a scalar bias.
///
/// The output at `(i, j)` is
///
/// ```text
/// bias + sum_{p,q} input[i + p][j + q] * kernel[p][q]
/// ```
///
/// with `p` in `0..kernel.len()` and `q` in `0..kernel[0].len()`.
///
/// # Arguments
///
/// * `input` - A rectangular single-channel map as rows of `f32` values.
/// * `kernel` - A rectangular kernel as rows of `f32` values.
/// * `bias` - A scalar added to every output cell.
///
/// # Returns
///
/// An output map of shape `(H - kh + 1) × (W - kw + 1)` where `H × W` is
/// the input shape and `kh × kw` is the kernel shape.
///
/// # Panics
///
/// Panics if any of the following preconditions is violated:
///
/// * `input` is empty, or any input row is empty.
/// * `input` rows have differing lengths (i.e. the map is not rectangular).
/// * `kernel` is empty, or any kernel row is empty.
/// * `kernel` rows have differing lengths.
/// * The input height or width is smaller than the kernel height or width.
///
/// # Note
///
/// Deep learning's "convolution" is technically a *cross-correlation*: the
/// kernel is not flipped before sliding. This function implements the
/// deep-learning convention directly.
pub fn cross_correlate(input: &[Vec<f32>], kernel: &[Vec<f32>], bias: f32) -> Vec<Vec<f32>> {
    assert!(!input.is_empty(), "input must contain at least one row");
    assert!(
        !input[0].is_empty(),
        "input rows must contain at least one value"
    );

    let in_h = input.len();
    let in_w = input[0].len();
    for (i, row) in input.iter().enumerate() {
        assert!(
            !row.is_empty(),
            "input row {i} is empty (all rows must be non-empty)"
        );
        assert_eq!(
            row.len(),
            in_w,
            "input row {i} has length {} but row 0 has length {in_w} (input must be rectangular)",
            row.len()
        );
    }

    assert!(!kernel.is_empty(), "kernel must contain at least one row");
    assert!(
        !kernel[0].is_empty(),
        "kernel rows must contain at least one value"
    );

    let k_h = kernel.len();
    let k_w = kernel[0].len();
    for (i, row) in kernel.iter().enumerate() {
        assert!(
            !row.is_empty(),
            "kernel row {i} is empty (all rows must be non-empty)"
        );
        assert_eq!(
            row.len(),
            k_w,
            "kernel row {i} has length {} but row 0 has length {k_w} (kernel must be rectangular)",
            row.len()
        );
    }

    assert!(
        in_h >= k_h && in_w >= k_w,
        "input ({in_h}×{in_w}) must be at least as large as kernel ({k_h}×{k_w})"
    );

    let out_h = in_h - k_h + 1;
    let out_w = in_w - k_w + 1;

    (0..out_h)
        .map(|i| {
            (0..out_w)
                .map(|j| {
                    let mut acc = bias;
                    for p in 0..k_h {
                        for q in 0..k_w {
                            acc += input[i + p][j + q] * kernel[p][q];
                        }
                    }
                    acc
                })
                .collect()
        })
        .collect()
}

/// Apply the ReLU activation in-place: negative values become zero.
pub fn relu_in_place(map: &mut [Vec<f32>]) {
    for row in map.iter_mut() {
        for v in row.iter_mut() {
            if *v < 0.0 {
                *v = 0.0;
            }
        }
    }
}

/// Return a new map with ReLU applied: negative values become zero.
pub fn relu(map: &[Vec<f32>]) -> Vec<Vec<f32>> {
    map.iter()
        .map(|row| row.iter().map(|&v| v.max(0.0)).collect())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cross_correlate_4x4_with_2x2() {
        // 4×4 input.
        let input = vec![
            vec![1.0, 2.0, 3.0, 4.0],
            vec![5.0, 6.0, 7.0, 8.0],
            vec![9.0, 10.0, 11.0, 12.0],
            vec![13.0, 14.0, 15.0, 16.0],
        ];

        // 2×2 kernel.
        let kernel = vec![vec![1.0, 0.0], vec![0.0, 1.0]];

        let output = cross_correlate(&input, &kernel, 0.0);

        // Output shape is 3×3.
        assert_eq!(output.len(), 3);
        assert_eq!(output[0].len(), 3);

        // Top-left: input[0][0] * 1 + input[0][1] * 0 + input[1][0] * 0 + input[1][1] * 1 = 1 + 6 = 7.
        assert_eq!(output[0][0], 7.0);
        // Top-middle: input[0][1] + input[1][2] = 2 + 7 = 9.
        assert_eq!(output[0][1], 9.0);
        // Top-right: input[0][2] + input[1][3] = 3 + 8 = 11.
        assert_eq!(output[0][2], 11.0);
        // Middle-left: input[1][0] + input[2][1] = 5 + 10 = 15.
        assert_eq!(output[1][0], 15.0);
        // Center: input[1][1] + input[2][2] = 6 + 11 = 17.
        assert_eq!(output[1][1], 17.0);
        // Middle-right: input[1][2] + input[2][3] = 7 + 12 = 19.
        assert_eq!(output[1][2], 19.0);
        // Bottom-left: input[2][0] + input[3][1] = 9 + 14 = 23.
        assert_eq!(output[2][0], 23.0);
        // Bottom-middle: input[2][1] + input[3][2] = 10 + 15 = 25.
        assert_eq!(output[2][1], 25.0);
        // Bottom-right: input[2][2] + input[3][3] = 11 + 16 = 27.
        assert_eq!(output[2][2], 27.0);
    }

    #[test]
    fn cross_correlate_with_bias() {
        let input = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let kernel = vec![vec![1.0, 1.0], vec![1.0, 1.0]];
        let output = cross_correlate(&input, &kernel, -5.0);
        assert_eq!(output.len(), 1);
        assert_eq!(output[0].len(), 1);
        // 1+2+3+4 - 5 = 5.
        assert_eq!(output[0][0], 5.0);
    }

    #[test]
    fn cross_correlate_1x1_kernel() {
        let input = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        let kernel = vec![vec![2.0]];
        let output = cross_correlate(&input, &kernel, 1.0);
        assert_eq!(output.len(), 2);
        assert_eq!(output[0].len(), 3);
        assert_eq!(output[0][0], 3.0); // 1*2 + 1
        assert_eq!(output[0][1], 5.0); // 2*2 + 1
        assert_eq!(output[0][2], 7.0); // 3*2 + 1
        assert_eq!(output[1][0], 9.0); // 4*2 + 1
        assert_eq!(output[1][1], 11.0); // 5*2 + 1
        assert_eq!(output[1][2], 13.0); // 6*2 + 1
    }

    #[test]
    fn cross_correlate_non_square_kernel() {
        let input = vec![
            vec![1.0, 2.0, 3.0, 4.0],
            vec![5.0, 6.0, 7.0, 8.0],
            vec![9.0, 10.0, 11.0, 12.0],
        ];
        // 2 rows × 3 cols kernel.
        let kernel = vec![vec![1.0, 0.0, -1.0], vec![0.0, 1.0, 0.0]];
        let output = cross_correlate(&input, &kernel, 0.0);
        // Output shape: (3 - 2 + 1) × (4 - 3 + 1) = 2 × 2.
        assert_eq!(output.len(), 2);
        assert_eq!(output[0].len(), 2);

        // Top-left: input[0][0]*1 + input[0][2]*(-1) + input[1][1]*1
        //         = 1 - 3 + 6 = 4.
        assert_eq!(output[0][0], 4.0);
        // Top-right: input[0][1]*1 + input[0][3]*(-1) + input[1][2]*1
        //          = 2 - 4 + 7 = 5.
        assert_eq!(output[0][1], 5.0);
        // Bottom-left: input[1][0]*1 + input[1][2]*(-1) + input[2][1]*1
        //            = 5 - 7 + 10 = 8.
        assert_eq!(output[1][0], 8.0);
        // Bottom-right: input[1][1]*1 + input[1][3]*(-1) + input[2][2]*1
        //             = 6 - 8 + 11 = 9.
        assert_eq!(output[1][1], 9.0);
    }

    #[test]
    fn relu_returns_non_negative() {
        let input = vec![vec![-2.0, 0.0, 3.0], vec![1.0, -5.0, 2.0]];
        let out = relu(&input);
        assert_eq!(out, vec![vec![0.0, 0.0, 3.0], vec![1.0, 0.0, 2.0],]);
    }

    #[test]
    fn relu_in_place_mutates() {
        let mut map = vec![vec![-1.0, 2.0], vec![0.0, -3.0]];
        relu_in_place(&mut map);
        assert_eq!(map, vec![vec![0.0, 2.0], vec![0.0, 0.0],]);
    }

    #[test]
    #[should_panic(expected = "input (2×2) must be at least as large as kernel (3×3)")]
    fn cross_correlate_kernel_larger_than_input() {
        let input = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let kernel = vec![
            vec![1.0, 2.0, 3.0],
            vec![4.0, 5.0, 6.0],
            vec![7.0, 8.0, 9.0],
        ];
        let _ = cross_correlate(&input, &kernel, 0.0);
    }

    #[test]
    #[should_panic(expected = "input must contain at least one row")]
    fn cross_correlate_empty_input() {
        let input: Vec<Vec<f32>> = vec![];
        let kernel = vec![vec![1.0]];
        let _ = cross_correlate(&input, &kernel, 0.0);
    }

    #[test]
    #[should_panic(
        expected = "input row 1 has length 1 but row 0 has length 2 (input must be rectangular)"
    )]
    fn cross_correlate_jagged_input() {
        let input = vec![vec![1.0, 2.0], vec![3.0]];
        let kernel = vec![vec![1.0]];
        let _ = cross_correlate(&input, &kernel, 0.0);
    }

    #[test]
    fn cross_correlate_rectangular_non_square_input() {
        // 3×5 input exercised to confirm rectangular-but-non-square maps work.
        let input = vec![
            vec![1.0, 2.0, 3.0, 4.0, 5.0],
            vec![6.0, 7.0, 8.0, 9.0, 10.0],
            vec![11.0, 12.0, 13.0, 14.0, 15.0],
        ];
        let kernel = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        let output = cross_correlate(&input, &kernel, 0.0);

        assert_eq!(output.len(), 2);
        assert_eq!(output[0].len(), 4);
        assert_eq!(output[0][0], 8.0); // input[0][0] + input[1][1]
        assert_eq!(output[0][1], 10.0);
        assert_eq!(output[0][2], 12.0);
        assert_eq!(output[0][3], 14.0);
        assert_eq!(output[1][0], 18.0);
        assert_eq!(output[1][1], 20.0);
        assert_eq!(output[1][2], 22.0);
        assert_eq!(output[1][3], 24.0); // input[1][3] + input[2][4]
    }
}
```
