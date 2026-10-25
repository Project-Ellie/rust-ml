//! Algebraic notation: the one canonical `h8` ↔ [`Move`] conversion.
//!
//! Before this module existed, the conversion lived in three places: a
//! private parser in `command.rs`, `format_move` in `fmt.rs`, and a
//! `notation()` helper in the historydemo binary. Copies of one logic
//! diverge over time. This module is the single implementation; every
//! caller delegates to it.
//!
//! The notation is column letter (`a`–`o`) followed by row number (`1`–`15`),
//! case-insensitive: `h8` is the center of the 15×15 board.

use engine::Move;
use thiserror::Error;

/// Why a token is not a valid algebraic square.
///
/// The `Display` text of each variant is part of the user interface: the
/// command language flattens these errors into [`crate::command::CommandError::Parse`]
/// and existing tests assert on the exact messages. Change a message only
/// together with those tests.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum NotationError {
    /// The token is too short to hold a column letter and a row number.
    #[error("Expected algebraic square like 'h8', got '{0}'")]
    NotAlgebraic(String),
    /// The first character is not a column letter in `a`–`o`.
    #[error("Expected column letter a-o, got '{letter}' in '{token}'")]
    BadColumn {
        /// The offending first character (already lowercased).
        letter: char,
        /// The token as the user typed it.
        token: String,
    },
    /// The part after the column letter is not a number.
    #[error("Expected row number 1-15, got '{digits}' in '{token}'")]
    BadRow {
        /// The offending row digits (already lowercased).
        digits: String,
        /// The token as the user typed it.
        token: String,
    },
    /// Column and row parsed, but the square is outside the 15×15 board.
    #[error("square '{0}' is off the board")]
    OffBoard(String),
}

/// Parse an algebraic square (`h8`) into a [`Move`].
///
/// The token is trimmed and lowercased first, so `H8`, ` h8 `, and `h8`
/// name the same square. Error messages quote the token as passed in, not
/// the normalized copy, so the user sees their own input.
pub fn parse_algebraic(token: &str) -> Result<Move, NotationError> {
    let text = token.trim().to_ascii_lowercase();
    // A square needs at least one column letter and one row digit.
    // `len` counts bytes; that is safe here because the column check below
    // rejects any non-ASCII first character before we slice bytes.
    if text.len() < 2 {
        return Err(NotationError::NotAlgebraic(token.to_string()));
    }
    let col_letter = text.chars().next().expect("len >= 2 has a first char");
    if !('a'..='o').contains(&col_letter) {
        return Err(NotationError::BadColumn {
            letter: col_letter,
            token: token.to_string(),
        });
    }
    // The column letter is ASCII (one byte), so slicing at byte 1 lands on
    // a char boundary.
    let row_digits = &text[1..];
    let row = row_digits
        .parse::<u8>()
        .map_err(|_| NotationError::BadRow {
            digits: row_digits.to_string(),
            token: token.to_string(),
        })?;
    let col = col_letter as u8 - b'a';
    // Rows are 1-based in notation and 0-based in `Move`. The `col > 14`
    // half of the check is unreachable — the letter check above already
    // bounds the column — but is kept so this function validates its full
    // contract on its own.
    if !(1..=15).contains(&row) || col > 14 {
        return Err(NotationError::OffBoard(token.to_string()));
    }
    // The range checks above are exactly `Move::new`'s preconditions, so
    // construction cannot fail.
    Ok(Move::new(row - 1, col).expect("row and column are in range"))
}

/// Format a [`Move`] as an algebraic square (`h8`).
///
/// This is the inverse of [`parse_algebraic`]: for every valid move,
/// `parse_algebraic(&algebraic(mv)) == Ok(mv)`.
#[must_use]
pub fn algebraic(mv: Move) -> String {
    // Column 0 prints as 'a'; row 0 prints as 1.
    format!("{}{}", (b'a' + mv.col()) as char, mv.row() + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::mv;

    #[test]
    fn parses_edge_and_center_squares() {
        // a1 and o15 are the two far corners; h8 is the center. If the
        // corners round-trip, every square in between does too.
        assert_eq!(parse_algebraic("a1"), Ok(mv(0, 0)));
        assert_eq!(parse_algebraic("o15"), Ok(mv(14, 14)));
        assert_eq!(parse_algebraic("h8"), Ok(mv(7, 7)));
    }

    #[test]
    fn is_case_insensitive() {
        assert_eq!(parse_algebraic("H8"), Ok(mv(7, 7)));
        assert_eq!(parse_algebraic("A1"), Ok(mv(0, 0)));
        assert_eq!(parse_algebraic("O15"), Ok(mv(14, 14)));
    }

    #[test]
    fn round_trips_edge_and_center_squares() {
        for token in ["a1", "o15", "h8"] {
            let parsed = parse_algebraic(token).unwrap();
            assert_eq!(algebraic(parsed), token);
        }
    }

    #[test]
    fn rejects_tokens_too_short() {
        let err = parse_algebraic("a").unwrap_err();
        assert_eq!(err, NotationError::NotAlgebraic("a".to_string()));
        assert_eq!(
            err.to_string(),
            "Expected algebraic square like 'h8', got 'a'"
        );
    }

    #[test]
    fn rejects_a_bad_column_letter() {
        let err = parse_algebraic("p8").unwrap_err();
        assert_eq!(
            err,
            NotationError::BadColumn {
                letter: 'p',
                token: "p8".to_string()
            }
        );
        assert_eq!(
            err.to_string(),
            "Expected column letter a-o, got 'p' in 'p8'"
        );
    }

    #[test]
    fn rejects_a_bad_row_number() {
        let err = parse_algebraic("h8h").unwrap_err();
        assert_eq!(
            err,
            NotationError::BadRow {
                digits: "8h".to_string(),
                token: "h8h".to_string()
            }
        );
        assert_eq!(
            err.to_string(),
            "Expected row number 1-15, got '8h' in 'h8h'"
        );
    }

    #[test]
    fn rejects_squares_off_the_board() {
        let err = parse_algebraic("h16").unwrap_err();
        assert_eq!(err, NotationError::OffBoard("h16".to_string()));
        assert_eq!(err.to_string(), "square 'h16' is off the board");

        let err = parse_algebraic("h0").unwrap_err();
        assert_eq!(err.to_string(), "square 'h0' is off the board");
    }
}
