//! All about opening in selfplay

use engine::Board;
use engine::{Color, Move, Placing3, PlayError, Swap2};
use rand::RngExt;
use rand::prelude::StdRng;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

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

/// One malformed line in a tournament opening list.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("opening list '{list}' line {line}: {reason}")]
pub struct OpeningParseError {
    /// Stable identity of the opening list.
    pub list: String,
    /// One-based physical line number.
    pub line: usize,
    /// Specific validation failure.
    pub reason: String,
}

/// Failure to load one or more tournament opening lists.
#[derive(Debug, Error)]
pub enum OpeningLoadError {
    /// No input paths were supplied.
    #[error("at least one tournament opening list is required")]
    Empty,
    /// An input file could not be read.
    #[error("failed to read opening list '{path}': {source}")]
    Read {
        /// Path that could not be read.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// A line inside a list could not be parsed.
    #[error(transparent)]
    Parse(#[from] OpeningParseError),
}

/// One validated offer from a tournament list.
#[derive(Debug, Clone)]
struct ListedOffer {
    list: String,
    index: usize,
    initial: [Move; 3],
}

/// A uniform pool of validated tournament opening offers.
///
/// Every non-empty line must contain at least three distinct, on-board
/// stones. Longer lines are validated in full, but only their first three
/// stones drive the Swap2 typestate: the ceremony consumes exactly three
/// stones, and the remainder of a long line is the source tournament's
/// pre-set continuation, not this generator's to replay.
#[derive(Debug, Clone)]
pub struct TournamentOpenings {
    offers: Vec<ListedOffer>,
}

impl TournamentOpenings {
    /// Parse a single tournament list from its text.
    ///
    /// `list` is the stable identity recorded in every sampled opening's
    /// provenance. The first malformed line fails the whole parse.
    ///
    /// # Errors
    /// Returns [`OpeningParseError`] when a line is empty, has too few
    /// pairs, an odd field count, a non-numeric coordinate, an off-board
    /// coordinate, or a duplicate coordinate within the same line.
    pub fn parse(list: impl Into<String>, text: &str) -> Result<Self, OpeningParseError> {
        let list = list.into();
        let mut offers = Vec::new();

        for (zero_based, line) in text.lines().enumerate() {
            let line_no = zero_based + 1;
            let err = |reason: String| OpeningParseError {
                list: list.clone(),
                line: line_no,
                reason,
            };

            let fields: Vec<_> = line.split(',').map(str::trim).collect();
            if fields.len() < 6 || fields.len() % 2 != 0 {
                return Err(err(
                    "expected at least three comma-separated x,y pairs".to_owned()
                ));
            }

            let mut stones = Vec::new();
            for pair in fields.as_chunks::<2>().0 {
                let x_text = pair[0];
                let y_text = pair[1];
                let x: i16 = x_text
                    .parse()
                    .map_err(|e| err(format!("invalid x coordinate '{x_text}': {e}")))?;
                let y: i16 = y_text
                    .parse()
                    .map_err(|e| err(format!("invalid y coordinate '{y_text}': {e}")))?;

                let col = 7 + x;
                let row = 7 + y;
                let board_range = 0..=14_i16;
                if !board_range.contains(&col) || !board_range.contains(&row) {
                    return Err(err(format!(
                        "coordinate {x},{y} is outside the 15 by 15 board"
                    )));
                }

                let mv = Move::new(row as u8, col as u8).ok_or_else(|| {
                    err(format!("coordinate {x},{y} is outside the 15 by 15 board"))
                })?;

                if stones.contains(&mv) {
                    return Err(err(format!("duplicate coordinate {x},{y}")));
                }
                stones.push(mv);
            }

            // Length checked above: at least three pairs were present.
            let initial: [Move; 3] = stones[..3].try_into().expect("length checked above");
            offers.push(ListedOffer {
                list: list.clone(),
                index: offers.len(),
                initial,
            });
        }

        if offers.is_empty() {
            return Err(OpeningParseError {
                list,
                line: 1,
                reason: "opening list is empty".to_owned(),
            });
        }

        Ok(Self { offers })
    }

    /// Load and concatenate one or more tournament opening lists.
    ///
    /// The display form of each path becomes the stable list identity
    /// recorded in provenance. Lists are concatenated in path order.
    ///
    /// # Errors
    /// Returns [`OpeningLoadError::Empty`] for an empty path slice,
    /// [`OpeningLoadError::Read`] for an unreadable file, or
    /// [`OpeningLoadError::Parse`] for the first malformed line.
    pub fn load<P: AsRef<Path>>(paths: &[P]) -> Result<Self, OpeningLoadError> {
        if paths.is_empty() {
            return Err(OpeningLoadError::Empty);
        }

        let mut combined = Vec::new();
        for path in paths {
            let path = path.as_ref();
            let text = fs::read_to_string(path).map_err(|e| OpeningLoadError::Read {
                path: path.to_owned(),
                source: e,
            })?;
            let identity = path.display().to_string();
            let mut parsed = Self::parse(identity, &text)?;
            combined.append(&mut parsed.offers);
        }

        Ok(Self { offers: combined })
    }

    /// Number of validated offers in the pool.
    #[must_use]
    pub fn len(&self) -> usize {
        self.offers.len()
    }

    /// Whether the pool contains no offers.
    ///
    /// Always `false` for a pool created through [`Self::parse`] or
    /// [`Self::load`].
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.offers.is_empty()
    }
}

impl OpeningSource for TournamentOpenings {
    fn opening(&mut self, rng: &mut StdRng) -> Opening {
        // Chapter 14 §3.3 perturbs a value-based argmax.
        // This source has no value model, so chapter 4 uses a seeded uniform
        // protocol choice as its exploration approximation, including both
        // color assignments.
        let offer = &self.offers[rng.random_range(0..self.offers.len())];

        let answer = match rng.random_range(0..3) {
            0 => OpeningAnswer::Take(Color::Black),
            1 => OpeningAnswer::Take(Color::White),
            _ => {
                let stones = draw_unique(rng, &offer.initial);
                OpeningAnswer::AddTwo {
                    stones,
                    final_choice: random_color(rng),
                }
            }
        };

        Opening::new(
            offer.initial,
            answer,
            OpeningProvenance::Listed {
                list: offer.list.clone(),
                index: offer.index,
            },
        )
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
