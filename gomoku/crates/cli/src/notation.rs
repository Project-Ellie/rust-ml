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
    /// Not in the form <letter><rownum>
    #[error("Expected algebraic structure like 'h8', got {0}.")]
    NotAlgebraic(String),
    /// The first letter is not a column letter
    #[error("Expected column letter a-o got '{letter}' in '{token}'.")]
    BadColumn { letter: char, token: String },
    /// The part after the column letter is not a number
    #[error("Expected row number 1-15, got '{digits}' in 'token'.")]
    BadRow { digits: String, token: String },
    /// Parsed successfully, but the position is outside the 15x15 boundary
    #[error("Square {0} is off the board.")]
    OffBoard(String),
}

/// Parse an algebraic square (`h8`) into a [`Move`].
///
/// The token is trimmed and lowercased first, so `H8`, ` h8 `, and `h8`
/// name the same square. Error messages quote the token as passed in, not
/// the normalized copy, so the user sees their own input.
pub fn parse_algebraic(token: &str) -> Result<Move, NotationError> {
    let text = token.trim().to_ascii_lowercase();
    if text.len() < 2 {
        return Err(NotationError::NotAlgebraic(token.to_string()));
    }
    let col_letter = text.chars().next().unwrap();
    if !('a'..='o').contains(&col_letter) {
        return Err(NotationError::BadColumn {
            letter: col_letter,
            token: token.to_string(),
        });
    }
    let row_digits = &text[1..];
    let row = row_digits
        .parse::<u8>()
        .map_err(|_| NotationError::BadRow {
            digits: row_digits.to_string(),
            token: token.to_string(),
        })?;
    let col = col_letter as u8 - b'a';
    if !(1..=15).contains(&row) || col > 14 {
        return Err(NotationError::OffBoard(token.to_string()));
    }
    Ok(Move::new(row - 1, col).expect("Row and column have been checked."))
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
    use testkit::mv;

    use super::*;

    #[test]
    fn parses_edge_cases_and_center_square() {
        let cases = [
            ("a1", (0, 0)),
            ("a15", (14, 0)),
            ("o15", (14, 14)),
            ("o1", (0, 14)),
            ("h8", (7, 7)),
        ];
        for (algebraic, (r, c)) in cases {
            assert_eq!(parse_algebraic(algebraic).unwrap(), mv(r, c));
        }
    }

    #[test]
    fn parse_is_case_insensitive() {
        let cases = [("A1", (0, 0)), ("O15", (14, 14)), ("H8", (7, 7))];
        for (algebraic, (r, c)) in cases {
            assert_eq!(parse_algebraic(algebraic).unwrap(), mv(r, c));
        }
    }

    #[test]
    fn round_trips_match() {
        let tokens = ["h1", "a8", "o15"];

        for token in tokens {
            let mv = parse_algebraic(token).unwrap();
            assert_eq!(algebraic(mv), token);
        }
    }

    #[test]
    fn parse_rejects_illegal_tokens() {
        assert!(matches!(
            parse_algebraic("a16").unwrap_err(),
            NotationError::OffBoard(_)
        ));
        assert!(matches!(
            parse_algebraic("h").unwrap_err(),
            NotationError::NotAlgebraic(_)
        ));
        assert!(matches!(
            parse_algebraic("x8").unwrap_err(),
            NotationError::BadColumn { .. }
        ));
        assert!(matches!(
            parse_algebraic("hk").unwrap_err(),
            NotationError::BadRow { .. }
        ));
    }
}
