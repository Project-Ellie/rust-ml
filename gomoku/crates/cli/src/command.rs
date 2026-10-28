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
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Play {
        mv: Move,
    },
    Step,
    Back {
        n: u32,
    },
    Forward {
        n: u32,
    },
    Var {
        index: usize,
    },
    Take {
        seat: SeatSel,
    },
    Release {
        seat: SeatSel,
    },
    SetSims {
        seat: SeatSel,
        n: u32,
    },
    /// End the current Swap2 placement round
    Done,
    /// Choose a color at the Swap2 choice phase
    ChooseColor {
        color: Color,
    },
    /// Decline the first Swap2 choicce and place two more stones
    More,
    New,
    Help,
    Quit,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeatSel {
    Black,
    White,
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
    /// The session was reset to its initial opening.
    Reset,
    /// A stone was placed during Swap2 opening
    StonePlaced { mv: Move, color: Color },
    /// `back` removed the last Swap2 placement.
    StoneRemoved { mv: Move },
    /// The Swap2 Opening entered a new phase.
    OpeningPhaseChanged { phase: OpeningPhase },
    /// The Swap2 opening finished; the game starts from its result.
    ///
    /// The payload is the chosen color and nothing else. The side to
    /// move is not carried: the engine guarantees that White moves
    /// first after the ceremony, because White always has fewer stones.
    OpeningFinished { chosen: Color },
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
            Event::Reset => write!(f, "Reset"),
            Event::StonePlaced { mv, color } => write!(f, "Placed {color:?} at {}", fmt_move(*mv)),
            Event::StoneRemoved { mv } => write!(f, "Removed stone from {}", fmt_move(*mv)),
            Event::OpeningPhaseChanged { phase } => write!(f, "Now in phase {phase:?}"),
            Event::OpeningFinished { chosen } => write!(f, "Opening finished, chose {chosen:?}"),
        }
    }
}

fn fmt_move(mv: Move) -> String {
    crate::fmt::format_move(mv)
}

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum CommandError {
    #[error("Parse error {0}")]
    Parse(String),
    #[error("Can't execute anything anymore. This game is over.")]
    GameOver,
    #[error("Illegal Move {0}")]
    IllegalMove(#[from] PlayError),
    #[error("choice out of bound: {index}. Options: {count}")]
    VarOutOfBounds {
        /// Requested index
        index: usize,
        /// Number of children available
        count: usize,
    },
    #[error("Tried to forward at a leaf. Choose a move instead.")]
    ForwardAtLeaf,
    #[error("Not at branch point. Simply move")]
    NotAtBranchPoint,
    #[error("Step for {seat:?} requires AI")]
    StepRequiresAi { seat: Color },
    #[error("Simulations must be greater than zero")]
    ZeroSims,
    #[error("No Swap2 opening is in progress.")]
    NotInOpening,
    #[error("The Swap2 opening is not at a placement phase.")]
    NotAtPlacementPhase,
    #[error("The Swap2 opening is not at a choice phase.")]
    NotAtChoicePhase,
    #[error("The Swap2 opening is not at the first Swap2 choice.")]
    NotAtFirstChoice,
    #[error("The Swap2 opening has no placement to remove.")]
    OpeningLogEmpty,
    #[error("The Swap2 opening is still in progress; choose a color first")]
    OpeningInProgress,
    #[error("The Swap2 opening round already has all stones it can hold")]
    OpeningRoundFull,
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

pub fn parse(input: &str) -> Result<Command, CommandError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(CommandError::Parse("empty input".to_string()));
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

        "help" | "h" | "?" => {
            ensure_no_more(&mut tokens, "help")?;
            Ok(Command::Help)
        }

        _ => {
            let mv = parse_algebraic(first).map_err(|err| CommandError::Parse(err.to_string()))?;
            ensure_no_more(&mut tokens, "play")?;

            Ok(Command::Play { mv })
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
}
