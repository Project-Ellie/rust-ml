//! Command language and events.
//!
//! The command language is the shared vocabulary between every front-end.
//! Text parsing is a convenience layer; the `Command` enum is the real
//! interface to `SessionCore`. Events are facts about what changed and are
//! the only output a front-end needs to render.

use crate::actor::OccupantKind;
use crate::history::HistoryError;
use crate::notation::parse_algebraic;
use engine::{Color, Move, PlayError, Status};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use thiserror::Error;

/// A command issued to `SessionCore`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Play a move at the cursor.
    Play {
        /// The move to play.
        mv: Move,
    },
    /// Let the AI play one move.
    Step,
    /// Go back `n` plies in the variation tree.
    Back {
        /// Number of plies to undo.
        n: u32,
    },
    /// Go forward `n` plies along the current line.
    Forward {
        /// Number of plies to redo.
        n: u32,
    },
    /// Choose a child variation by index.
    Var {
        /// Zero-based child index.
        index: usize,
    },
    /// Make a seat human.
    Take {
        /// Seat or seats to take.
        seat: SeatSel,
    },
    /// Release a seat to the AI.
    Release {
        /// Seat or seats to release.
        seat: SeatSel,
    },
    /// Set the simulation count for a seat.
    SetSims {
        /// Seat or seats to configure.
        seat: SeatSel,
        /// Number of MCTS simulations.
        n: u32,
    },
    /// End the current Swap2 placement round.
    Done,
    /// Choose a color at the Swap2 choice phase.
    ChooseColor {
        /// The chosen color.
        color: Color,
    },
    /// Decline the first Swap2 choice and place two more stones.
    More,
    /// Reset the session to its opening.
    New,
    /// Show the first move of the loaded puzzle's claimed line.
    Hint,
    /// Show the full claimed line of the loaded puzzle.
    Reveal,
    /// Load the next puzzle in the pack as a fresh tree root.
    ///
    /// Navigation clamps: at the last puzzle this is an error, not a
    /// wrap-around.
    NextPuzzle,
    /// Load the previous puzzle in the pack as a fresh tree root.
    ///
    /// Navigation clamps: at the first puzzle this is an error, not a
    /// wrap-around.
    PrevPuzzle,
    /// Inspect stored original concessions; indices are one-based.
    Decision {
        /// Event number, or all events (latest for a candidate preview).
        event: Option<usize>,
        /// Candidate number to preview, if requested.
        candidate: Option<usize>,
    },
    /// Show help.
    Help,
    /// Leave the session.
    Quit,
    /// Save the study through a filesystem-capable shell
    Save {
        /// Literal destination path
        path: PathBuf,
    },
    /// Load a study through a filesystem-capable shell.
    Load {
        /// Literal source path
        path: PathBuf,
    },
    /// Install a trained net model (a `nettrain` run directory holding
    /// `model.mpk` and `run.json`) as the evaluator of every AI seat.
    /// Served by the shell; runtime-only, never serialized.
    LoadModel {
        /// Literal run-directory path
        path: PathBuf,
    },
}

/// Phase of a Swap2 opening ceremony.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpeningPhase {
    /// Player A places three stones: black-white-black.
    Placing3,
    /// Player B chooses a color or the opportunity to place two more stones: white-black.
    FirstChoice,
    /// Player B places two more stones: white-black.
    Placing2,
    /// Player A chooses a color.
    FinalChoice,
}

/// One seat, or both seats at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeatSel {
    /// The Black seat.
    Black,
    /// The White seat.
    White,
    /// Both seats.
    Both,
}

impl fmt::Display for SeatSel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SeatSel::Black => write!(f, "black"),
            SeatSel::White => write!(f, "white"),
            SeatSel::Both => write!(f, "both"),
        }
    }
}

/// A fact emitted by `SessionCore::execute` describing what changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A stone was played at the cursor.
    Moved {
        /// The move played.
        mv: Move,
        /// Who played it.
        by: OccupantKind,
    },
    /// A new sibling branch was created at the cursor's parent.
    Branched {
        /// The available child moves at the branch point.
        options: Vec<Move>,
    },
    /// The cursor moved to a different ply.
    CursorMoved {
        /// The new cursor ply.
        ply: u32,
    },
    /// `Forward` stopped at a branch point and needs a `Var` choice.
    BranchPoint {
        /// Index/move pairs for the available children.
        options: Vec<(usize, Move)>,
    },
    /// A seat's occupant changed.
    SeatChanged {
        /// The seat that changed.
        seat: Color,
        /// The new occupant kind.
        now: OccupantKind,
    },
    /// A seat's search configuration changed.
    ConfigChanged {
        /// The seat that changed.
        seat: Color,
        /// The new simulation count.
        sims: u32,
    },
    /// The game ended at the cursor.
    GameOver {
        /// Final status.
        result: Status,
    },
    /// The side to move made a heuristic concession without placing a stone.
    Resigned {
        /// Side that conceded.
        by: Color,
        /// Number of top candidates with verified opponent refutations.
        candidates: usize,
    },
    /// A human explicitly resumed the conceding side at this position.
    Resumed {
        /// Side whose historical concession no longer blocks play.
        by: Color,
    },
    /// Display original decision evidence without embedding float-bearing audits.
    DecisionReport {
        /// One-based event selection, or all events.
        event: Option<usize>,
        /// One-based candidate selection, if any.
        candidate: Option<usize>,
    },
    /// The session was reset to its initial opening.
    Reset,
    /// A stone was placed during Swap2 opening.
    StonePlaced {
        /// The placed stone.
        mv: Move,
        /// Its color.
        color: Color,
    },
    /// `back` removed the last Swap2 placement.
    StoneRemoved {
        /// The removed stone.
        mv: Move,
    },
    /// The Swap2 Opening entered a new phase.
    OpeningPhaseChanged {
        /// The new phase.
        phase: OpeningPhase,
    },
    /// The Swap2 opening finished; the game starts from its result.
    ///
    /// The payload is the chosen color and nothing else. The side to
    /// move is not carried: the engine guarantees that White moves
    /// first after the ceremony, because White always has fewer stones.
    OpeningFinished {
        /// The chosen color.
        chosen: Color,
    },
    /// The first move of the loaded puzzle's claimed line.
    Hint {
        /// First move of the claimed line.
        mv: Move,
    },
    /// The full claimed line of the loaded puzzle.
    SolutionRevealed {
        /// Claimed line: move and side for each ply.
        line: Vec<(Move, Color)>,
    },
    /// A puzzle was loaded as a fresh variation tree root.
    PuzzleLoaded {
        /// Zero-based index within the pack.
        index: usize,
        /// Number of puzzles in the pack.
        count: usize,
        /// Side to move (the attacker).
        attacker: Color,
    },
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Event::Moved { mv, by } => {
                write!(f, "Moved {} by {by:?}", fmt_move(*mv))
            }
            Event::Branched { options } => {
                let opts = options
                    .iter()
                    .map(|m| fmt_move(*m))
                    .collect::<Vec<_>>()
                    .join(" ");
                write!(f, "Branched: {opts}")
            }
            Event::CursorMoved { ply } => write!(f, "Cursor moved to ply {ply}"),
            Event::BranchPoint { options } => {
                let opts = options
                    .iter()
                    .map(|(i, m)| format!("{i}: {}", fmt_move(*m)))
                    .collect::<Vec<_>>()
                    .join(" ");
                write!(f, "Branch point: {opts}")
            }
            Event::SeatChanged { seat, now } => write!(f, "{seat:?} seat is now {now:?}"),
            Event::ConfigChanged { seat, sims } => {
                write!(f, "{seat:?} simulations set to {sims}")
            }
            Event::GameOver { result } => write!(f, "Game over: {result:?}"),
            Event::Resigned { by, candidates } => write!(
                f,
                "{by:?} resigned: {candidates} top candidates VCF-refuted (heuristic; other moves unchecked)"
            ),
            Event::Resumed { by } => write!(f, "Resumed {by:?} after heuristic concession"),
            Event::DecisionReport { .. } => write!(f, "Original decision report"),
            Event::Reset => write!(f, "Reset"),
            Event::StonePlaced { mv, color } => write!(f, "Placed {color:?} at {}", fmt_move(*mv)),
            Event::StoneRemoved { mv } => write!(f, "Removed stone from {}", fmt_move(*mv)),
            Event::OpeningPhaseChanged { phase } => write!(f, "Now in phase {phase:?}"),
            Event::OpeningFinished { chosen } => write!(f, "Opening finished, chose {chosen:?}"),
            Event::Hint { mv } => write!(f, "hint: the line starts at {}", fmt_move(*mv)),
            Event::SolutionRevealed { line } => {
                let text = line
                    .iter()
                    .map(|(mv, side)| format!("{} {side:?}", fmt_move(*mv)))
                    .collect::<Vec<_>>()
                    .join(" → ");
                write!(f, "solution: {text}")
            }
            Event::PuzzleLoaded {
                index,
                count,
                attacker,
            } => {
                // Display is 1-based; the event fields stay 0-based for
                // machine consumers.
                write!(f, "puzzle {} of {count} · {attacker:?} to move", index + 1)
            }
        }
    }
}

fn fmt_move(mv: Move) -> String {
    crate::fmt::format_move(mv)
}

/// Why a command could not be executed.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum CommandError {
    /// Text input did not match any command grammar.
    #[error("Parse error {0}")]
    Parse(String),
    /// The game is already over.
    #[error("Can't execute anything anymore. This game is over.")]
    GameOver,
    /// The move is illegal in the cursor position.
    #[error("Illegal Move {0}")]
    IllegalMove(#[from] PlayError),
    /// The chosen variation index does not exist.
    #[error("choice out of bound: {index}. Options: {count}")]
    VarOutOfBounds {
        /// Requested index.
        index: usize,
        /// Number of children available.
        count: usize,
    },
    /// `forward` reached a leaf.
    #[error("Tried to forward at a leaf. Choose a move instead.")]
    ForwardAtLeaf,
    /// `var` was used when the cursor is not at a branch point.
    #[error("Not at branch point. Simply move")]
    NotAtBranchPoint,
    /// `step` was issued for a human seat.
    #[error("Step for {seat:?} requires AI")]
    StepRequiresAi {
        /// The seat that must be AI.
        seat: Color,
    },
    /// The simulation count must be positive.
    #[error("Simulations must be greater than zero")]
    ZeroSims,
    /// No Swap2 opening is in progress.
    #[error("No Swap2 opening is in progress.")]
    NotInOpening,
    /// The Swap2 opening is not at a placement phase.
    #[error("The Swap2 opening is not at a placement phase.")]
    NotAtPlacementPhase,
    /// The Swap2 opening is not at a choice phase.
    #[error("The Swap2 opening is not at a choice phase.")]
    NotAtChoicePhase,
    /// `more` was used outside the first Swap2 choice.
    #[error("The Swap2 opening is not at the first Swap2 choice.")]
    NotAtFirstChoice,
    /// `back` has nothing to remove in the opening log.
    #[error("The Swap2 opening has no placement to remove.")]
    OpeningLogEmpty,
    /// A game command was issued while the Swap2 ceremony is open.
    #[error("The Swap2 opening is still in progress; choose a color first")]
    OpeningInProgress,
    /// A placement would exceed the round's stone count.
    #[error("The Swap2 opening round already has all stones it can hold")]
    OpeningRoundFull,
    /// A puzzle command was issued but no puzzle pack is loaded.
    #[error("No puzzle loaded — start with 'gomoku play --puzzle <pack.json>'")]
    NoPuzzleLoaded,
    /// The loaded puzzle has no claimed solution line.
    #[error("Puzzle {} of {count} has no claimed line", index + 1)]
    NoSolutionLine {
        /// Zero-based index of the loaded puzzle.
        index: usize,
        /// Number of puzzles in the pack.
        count: usize,
    },
    /// `next` at the last puzzle — navigation clamps.
    #[error("Already at the last puzzle ({count} of {count})")]
    AtLastPuzzle {
        /// Number of puzzles in the pack.
        count: usize,
    },
    /// `prev` at the first puzzle — navigation clamps.
    #[error("Already at the first puzzle (1 of {count})")]
    AtFirstPuzzle {
        /// Number of puzzles in the pack.
        count: usize,
    },
    /// An event or candidate index is outside the stored audit.
    #[error("decision {kind} {index} out of range (1..={count})")]
    DecisionOutOfBounds {
        /// Selection kind.
        kind: &'static str,
        /// Requested one-based index.
        index: usize,
        /// Available items.
        count: usize,
    },
    /// Legacy records contain no original certificate to preview.
    #[error("Original decision log/proofs unavailable (legacy record); cannot preview.")]
    DecisionUnavailable,
    /// Save and load make no sense in a script, or do they...?
    #[error("save/load must be executed through the shell")]
    ShellOnly,
}

impl From<HistoryError> for CommandError {
    fn from(err: HistoryError) -> Self {
        match err {
            HistoryError::IllegalMove(e) => CommandError::IllegalMove(e),
            HistoryError::VarOutOfBounds { index, child_count } => CommandError::VarOutOfBounds {
                index,
                count: child_count,
            },
        }
    }
}

/// Parse one line of text into a [`Command`].
///
/// The grammar is case-insensitive. A bare algebraic square (`h8`) is
/// shorthand for `play h8`.
///
/// # Errors
/// Returns [`CommandError::Parse`] for empty, unknown, or malformed input.
pub fn parse(input: &str) -> Result<Command, CommandError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(CommandError::Parse("empty input".to_string()));
    }

    let split = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
    let verb = &trimmed[..split];
    if verb.eq_ignore_ascii_case("save") || verb.eq_ignore_ascii_case("load") {
        let remainder = trimmed[split..].trim();
        if remainder.is_empty() {
            return Err(CommandError::Parse(format!("{verb} requires a path")));
        }
        let path = std::path::PathBuf::from(remainder);
        return Ok(if verb.eq_ignore_ascii_case("save") {
            Command::Save { path }
        } else {
            Command::Load { path }
        });
    }
    if verb.eq_ignore_ascii_case("model") {
        let remainder = trimmed[split..].trim();
        if remainder.is_empty() {
            return Err(CommandError::Parse(
                "model requires a nettrain run directory path".to_string(),
            ));
        }
        return Ok(Command::LoadModel {
            path: std::path::PathBuf::from(remainder),
        });
    }

    let lower = trimmed.to_ascii_lowercase();
    let mut tokens = lower.split_whitespace();
    let first = tokens.next().expect("There's at least one");

    match first {
        "quit" | "q" | "exit" => {
            ensure_no_more(&mut tokens, "quit")?;
            Ok(Command::Quit)
        }

        "play" => {
            let token = parse_required_token(&mut tokens, "square")?;
            let mv = parse_algebraic(&token).map_err(|err| CommandError::Parse(err.to_string()))?;
            ensure_no_more(&mut tokens, "play")?;
            Ok(Command::Play { mv })
        }

        "done" => {
            ensure_no_more(&mut tokens, "done")?;
            Ok(Command::Done)
        }

        "black" => {
            ensure_no_more(&mut tokens, "black")?;
            Ok(Command::ChooseColor {
                color: Color::Black,
            })
        }

        "white" => {
            ensure_no_more(&mut tokens, "white")?;
            Ok(Command::ChooseColor {
                color: Color::White,
            })
        }

        "more" => {
            ensure_no_more(&mut tokens, "more")?;
            Ok(Command::More)
        }

        "step" => {
            ensure_no_more(&mut tokens, "step")?;
            Ok(Command::Step)
        }

        "back" => {
            let n = parse_optional_u32(&mut tokens, 1, "back")?;
            Ok(Command::Back { n })
        }

        "forward" => {
            let n = parse_optional_u32(&mut tokens, 1, "forward")?;
            Ok(Command::Forward { n })
        }

        "var" => {
            let index = parse_required_usize(&mut tokens, "var")?;
            Ok(Command::Var { index })
        }

        "take" => {
            let seat_sel = parse_required_seat(&mut tokens, "take")?;
            ensure_no_more(&mut tokens, "take")?;
            Ok(Command::Take { seat: seat_sel })
        }

        "release" => {
            let seat_sel = parse_required_seat(&mut tokens, "release")?;
            ensure_no_more(&mut tokens, "release")?;
            Ok(Command::Release { seat: seat_sel })
        }

        "sims" => {
            let seat = parse_required_seat(&mut tokens, "release")?;
            let n = parse_required_u32(&mut tokens, "sims")?;
            Ok(Command::SetSims { seat, n })
        }

        "new" => {
            ensure_no_more(&mut tokens, "new")?;
            Ok(Command::New)
        }

        "decision" => {
            let args: Vec<_> = tokens.collect();
            let positive = |s: &str| -> Result<usize, CommandError> {
                s.bytes()
                    .all(|b| b.is_ascii_digit())
                    .then(|| s.parse::<usize>().ok())
                    .flatten()
                    .filter(|n| *n > 0)
                    .ok_or_else(|| {
                        CommandError::Parse("decision expects a positive one-based index".into())
                    })
            };
            let (candidate, event) = match args.as_slice() {
                [] => (None, None),
                [n] => (Some(positive(n)?), None),
                ["event", e] => (None, Some(positive(e)?)),
                [n, "event", e] => (Some(positive(n)?), Some(positive(e)?)),
                _ => return Err(CommandError::Parse("usage: decision [N] [event E]".into())),
            };
            Ok(Command::Decision { event, candidate })
        }

        "help" | "h" | "?" => {
            ensure_no_more(&mut tokens, "help")?;
            Ok(Command::Help)
        }

        "hint" => {
            ensure_no_more(&mut tokens, "hint")?;
            Ok(Command::Hint)
        }

        "reveal" => {
            ensure_no_more(&mut tokens, "reveal")?;
            Ok(Command::Reveal)
        }

        "next" => {
            ensure_no_more(&mut tokens, "next")?;
            Ok(Command::NextPuzzle)
        }

        "prev" => {
            ensure_no_more(&mut tokens, "prev")?;
            Ok(Command::PrevPuzzle)
        }

        _ => {
            let mv = parse_algebraic(first).map_err(|err| CommandError::Parse(err.to_string()))?;
            ensure_no_more(&mut tokens, "play")?;

            Ok(Command::Play { mv })
        }
    }
}

#[cfg(test)]
mod decision_tests {
    use super::*;

    #[test]
    fn decision_parser_strict_selection() {
        for (text, event, candidate) in [
            ("decision", None, None),
            ("decision 1", None, Some(1)),
            ("decision event 2", Some(2), None),
            ("decision 5 event 1", Some(1), Some(5)),
            ("DECISION 2 EVENT 3", Some(3), Some(2)),
        ] {
            assert_eq!(
                parse(text),
                Ok(Command::Decision { event, candidate }),
                "{text}"
            );
        }
        for text in [
            "decision 0",
            "decision event 0",
            "decision -1",
            "decision +1",
            "decision 1 event 0",
            "decision 9999999999999999999999999999999",
            "decision x",
            "decision 1 event",
            "decision event 1 2",
            "decision 1 extra",
            "decisions",
        ] {
            assert!(parse(text).is_err(), "{text}");
        }
    }
}

fn ensure_no_more<'a>(
    tokens: &mut impl Iterator<Item = &'a str>,
    cmd: &str,
) -> Result<(), CommandError> {
    if let Some(extra) = tokens.next() {
        return Err(CommandError::Parse(format!(
            "{cmd} takes no arguments, got extra '{extra}'"
        )));
    }
    Ok(())
}

fn parse_required_token<'a>(
    tokens: &mut impl Iterator<Item = &'a str>,
    name: &str,
) -> Result<String, CommandError> {
    tokens
        .next()
        .map(|s| s.to_string())
        .ok_or_else(|| CommandError::Parse(format!("missing required token {}", name)))
}

fn parse_optional_u32<'a>(
    tokens: &mut impl Iterator<Item = &'a str>,
    default: u32,
    cmd: &str,
) -> Result<u32, CommandError> {
    match tokens.next() {
        None => Ok(default),
        Some(t) => {
            let n = t.parse::<u32>().map_err(|_| {
                CommandError::Parse(format!("{cmd} expects a non-negative number, got '{t}'"))
            })?;
            ensure_no_more(tokens, cmd)?;
            Ok(n)
        }
    }
}

fn parse_required_u32<'a>(
    tokens: &mut impl Iterator<Item = &'a str>,
    cmd: &str,
) -> Result<u32, CommandError> {
    let t = tokens
        .next()
        .ok_or_else(|| CommandError::Parse(format!("{cmd} expects a number")))?;
    let n = t.parse::<u32>().map_err(|_| {
        CommandError::Parse(format!("{cmd} expects a non-negative number, got '{t}'"))
    })?;
    ensure_no_more(tokens, cmd)?;
    Ok(n)
}

fn parse_required_usize<'a>(
    tokens: &mut impl Iterator<Item = &'a str>,
    cmd: &str,
) -> Result<usize, CommandError> {
    let t = tokens
        .next()
        .ok_or_else(|| CommandError::Parse(format!("{cmd} expects an index")))?;
    let n = t.parse::<usize>().map_err(|_| {
        CommandError::Parse(format!("{cmd} expects a non-negative index, got '{t}'"))
    })?;
    ensure_no_more(tokens, cmd)?;
    Ok(n)
}

fn parse_required_seat<'a>(
    tokens: &mut impl Iterator<Item = &'a str>,
    cmd: &str,
) -> Result<SeatSel, CommandError> {
    let t = tokens
        .next()
        .ok_or_else(|| CommandError::Parse(format!("{cmd} expects a seat: black|white|both")))?;
    match t {
        "black" => Ok(SeatSel::Black),
        "white" => Ok(SeatSel::White),
        "both" => Ok(SeatSel::Both),
        _ => Err(CommandError::Parse(format!(
            "{cmd} expects a seat: black|white|both, got '{t}'"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use testkit::mv;

    #[test]
    fn parse_play_rejects_a_color_token() {
        // Ceremony placements follow strict alternation; there is no
        // color token on `play` anymore, on either form of the
        // command. The standalone `black`/`white` choice commands
        // still parse (see parse_swap2_commands).
        let err = parse("play h8 white").unwrap_err();
        assert_eq!(
            err,
            CommandError::Parse("play takes no arguments, got extra 'white'".to_string())
        );
        assert!(parse("play h8 BLACK").is_err());
        assert!(parse("h8 white").is_err());
        assert!(parse("play h8 red").is_err());
        assert!(parse("play h8 white extra").is_err());
    }

    #[test]
    fn parse_quit() {
        assert!(parse("q").is_ok());
    }
    #[test]
    fn parse_play_bare_numbers() {
        assert_eq!(parse("h8"), Ok(Command::Play { mv: mv(7, 7) }));
        assert_eq!(parse("o1"), Ok(Command::Play { mv: mv(0, 14) }));
        assert_eq!(parse("a15"), Ok(Command::Play { mv: mv(14, 0) }));
    }

    #[test]
    fn parse_play_explicit() {
        assert_eq!(parse("play H8"), Ok(Command::Play { mv: mv(7, 7) }));
        assert_eq!(parse("PLAy o1"), Ok(Command::Play { mv: mv(0, 14) }));
    }

    #[test]
    fn parse_step() {
        assert_eq!(parse("step"), Ok(Command::Step));
        assert_eq!(parse("STEP"), Ok(Command::Step));
    }

    #[test]
    fn parse_back_default_and_explicit() {
        assert_eq!(parse("back"), Ok(Command::Back { n: 1 }));
        assert_eq!(parse("BACK 3"), Ok(Command::Back { n: 3 }));
        assert_eq!(parse("back 0"), Ok(Command::Back { n: 0 }));
    }

    #[test]
    fn parse_forward_default_and_explicit() {
        assert_eq!(parse("forward"), Ok(Command::Forward { n: 1 }));
        assert_eq!(parse("forward  5"), Ok(Command::Forward { n: 5 }))
    }

    #[test]
    fn parse_var() {
        assert_eq!(parse("var 0"), Ok(Command::Var { index: 0 }));
        assert_eq!(parse("VAR 2"), Ok(Command::Var { index: 2 }))
    }

    #[test]
    fn parse_take_and_release() {
        assert_eq!(
            parse("take black"),
            Ok(Command::Take {
                seat: SeatSel::Black
            })
        );
        assert_eq!(
            parse("take white"),
            Ok(Command::Take {
                seat: SeatSel::White
            })
        );
        assert_eq!(
            parse("take both"),
            Ok(Command::Take {
                seat: SeatSel::Both
            })
        );
        assert_eq!(
            parse("release black"),
            Ok(Command::Release {
                seat: SeatSel::Black
            })
        );
        assert_eq!(
            parse("RELEASE WHITE"),
            Ok(Command::Release {
                seat: SeatSel::White
            })
        );
    }

    #[test]
    fn parse_sims() {
        assert_eq!(
            parse("sims black 800"),
            Ok(Command::SetSims {
                seat: SeatSel::Black,
                n: 800,
            })
        );
        assert_eq!(
            parse("sims white 50"),
            Ok(Command::SetSims {
                seat: SeatSel::White,
                n: 50,
            })
        );
    }

    #[test]
    fn parse_new_help_quit() {
        assert_eq!(parse("new"), Ok(Command::New));
        assert_eq!(parse("h"), Ok(Command::Help));
        assert_eq!(parse("help"), Ok(Command::Help));
        assert_eq!(parse("?"), Ok(Command::Help));
        assert_eq!(parse("q"), Ok(Command::Quit));
        assert_eq!(parse("quit"), Ok(Command::Quit));
        assert_eq!(parse("exit"), Ok(Command::Quit));
    }

    #[test]
    fn parse_errors() {
        assert!(parse("").is_err());
        assert!(parse("a").is_err());
        assert!(parse("h8h").is_err());
        assert!(parse("play a").is_err());
        assert!(parse("back x").is_err());
        assert!(parse("var").is_err());
        assert!(parse("take red").is_err());
        assert!(parse("sims black").is_err());
        assert!(parse("sims black 100 extra").is_err());
        assert!(parse("unknown").is_err());
        assert!(parse("step extra").is_err());
    }

    #[test]
    fn parse_out_of_bounds_move_is_error() {
        assert!(parse("h16").is_err());
        assert!(parse("p8").is_err());
    }

    #[test]
    fn parse_supports_swap2_commands() {
        assert_eq!(parse("done"), Ok(Command::Done));
        assert_eq!(parse("DONE"), Ok(Command::Done));
        assert_eq!(
            parse("black"),
            Ok(Command::ChooseColor {
                color: Color::Black
            })
        );
        assert_eq!(
            parse("white"),
            Ok(Command::ChooseColor {
                color: Color::White
            })
        );
        assert_eq!(parse("more"), Ok(Command::More));
        assert!(parse("done now").is_err());
        assert!(parse("black now").is_err());
        assert!(parse("more 2").is_err());
    }

    #[test]
    fn parse_puzzle_commands() {
        assert_eq!(parse("hint"), Ok(Command::Hint));
        assert_eq!(parse("REVeal"), Ok(Command::Reveal));
        assert_eq!(parse("next"), Ok(Command::NextPuzzle));
        assert_eq!(parse("prev"), Ok(Command::PrevPuzzle));
        assert!(parse("hint x").is_err());
        assert!(parse("next 2").is_err());
    }

    #[test]
    fn file_paths_preserve_case_and_internal_spaces() {
        assert_eq!(
            parse("  SaVe   Games/My  Study.JSON \t").unwrap(),
            Command::Save {
                path: "Games/My  Study.JSON".into()
            }
        );
        assert_eq!(
            parse("LoAd\tMixed Case/a.json").unwrap(),
            Command::Load {
                path: "Mixed Case/a.json".into()
            }
        );
        assert!(parse("save \t").is_err());
        assert!(parse("load").is_err());
        assert_eq!(
            parse("save ~/x.json").unwrap(),
            Command::Save {
                path: "~/x.json".into()
            }
        );
    }

    #[test]
    fn model_command_parses_a_run_directory_path() {
        assert_eq!(
            parse("model artifacts/gomoku/runs/first-model").unwrap(),
            Command::LoadModel {
                path: "artifacts/gomoku/runs/first-model".into()
            }
        );
        // Like save/load: case-insensitive verb, internal spaces kept.
        assert_eq!(
            parse("Model  My Runs/run 1").unwrap(),
            Command::LoadModel {
                path: "My Runs/run 1".into()
            }
        );
        assert!(parse("model").is_err());
        assert!(parse("model \t").is_err());
    }
}
