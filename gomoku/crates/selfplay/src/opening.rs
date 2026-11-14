//! All about opening in selfplay

use engine::Board;
use engine::{Color, Move, Placing3, PlayError, Swap2};
use rand::RngExt;
use rand::prelude::StdRng;

/// A complete Swap2 offer and its answers
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Opening {
    /// Origin of this opening for later corpus metadata.
    pub provenance: OpeningProvenance,
    /// Initial cells in placement order
    pub initial: [Move; 3],
    /// The answering side's choice
    pub answer: OpeningAnswer,
}

/// The answering side's choice during Swap2
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OpeningAnswer {
    /// Finish after three stones by taking this color.
    Take(Color),
    /// Add two more stones
    AddTwo {
        /// The two new cells, in placement order.
        stones: [Move; 2],
        /// The color taken after the five-stone offer.
        final_choice: Color,
    },
}

/// Where an opening came from
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OpeningProvenance {
    /// Stones and answers drawn from the game's seeded RNG
    Generated,
    /// An opening taken from an external ordered list.
    Listed {
        /// Stable identity of the opening list.
        list: String,
        /// Zero-based position within that list.
        index: usize,
    },
}

impl Opening {
    /// Describe an opening withou executing it.
    #[must_use]
    pub const fn new(
        initial: [Move; 3],
        answer: OpeningAnswer,
        provenance: OpeningProvenance,
    ) -> Self {
        Self {
            provenance,
            initial,
            answer,
        }
    }

    /// Execute this opening through the engine's Swap2 typestate
    pub fn board(&self) -> Result<Board, PlayError> {
        let mut placing = Swap2::<Placing3>::new();
        placing.place(self.initial[0], Color::Black)?;
        placing.place(self.initial[1], Color::White)?;
        placing.place(self.initial[2], Color::Black)?;
        let first_choice = placing.finish()?;

        match self.answer {
            OpeningAnswer::Take(Color::Black) => Ok(first_choice.take_black()),
            OpeningAnswer::Take(Color::White) => Ok(first_choice.take_white()),
            OpeningAnswer::AddTwo {
                stones,
                final_choice,
            } => {
                let mut placing = first_choice.place_two_more();
                placing.place(stones[0], Color::White)?;
                placing.place(stones[1], Color::Black)?;
                let final_state = placing.finish()?;
                Ok(match final_choice {
                    Color::Black => final_state.take_black(),
                    Color::White => final_state.take_white(),
                })
            }
        }
    }
}

/// Pluggable source of complete Swap2 opening descriptions
///
/// The runner supplies the same seeed TNG later used for move selection,
/// so a game still has exactly one RNG stream an one identity: its seed.
pub trait OpeningSource {
    /// produce the next opening description
    fn opening(&mut self, rng: &mut StdRng) -> Opening;
}

/// An opening source that draws unique cells and answers from the game RNG
#[derive(Debug, Default)]
pub struct GeneratedOpening;

impl OpeningSource for GeneratedOpening {
    fn opening(&mut self, rng: &mut StdRng) -> Opening {
        let initial = draw_unique(rng, &[]);
        let answer = match rng.random_range(0..3) {
            0 => OpeningAnswer::Take(Color::Black),
            1 => OpeningAnswer::Take(Color::White),
            _ => {
                let stones = draw_unique(rng, &initial);
                OpeningAnswer::AddTwo {
                    stones,
                    final_choice: random_color(rng),
                }
            }
        };

        Opening::new(initial, answer, OpeningProvenance::Generated)
    }
}

fn random_color(rng: &mut StdRng) -> Color {
    if rng.random_range(0..2) == 0 {
        Color::Black
    } else {
        Color::White
    }
}

fn draw_unique<const N: usize>(rng: &mut StdRng, excluded: &[Move]) -> [Move; N] {
    let mut available: Vec<_> = (4..11)
        .flat_map(|row| {
            (4..11).map(move |col| Move::new(row, col).expect("both coordinates are inside board"))
        })
        .filter(|mv| !excluded.contains(mv))
        .collect();
    std::array::from_fn(|_| {
        let selected = rng.random_range(0..available.len());
        available.swap_remove(selected)
    })
}
