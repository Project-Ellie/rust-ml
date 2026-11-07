//! Terminal Gomoku client library.
//!
//! The `cli` crate is split into a headless library (history, command,
//! session, actor) and a thin binary front-end. The `tui` module holds
//! the rendered board, side panel, and input handling.

#![deny(missing_docs)]

/// Seat occupants and AI configuration.
pub mod actor;
/// Command language and events.
pub mod command;
/// Text-formatting helpers.
pub mod fmt;
/// load and save files
pub mod game_file;
/// Persistent variation tree.
pub mod history;
/// Algebraic notation.
pub mod notation;
/// Swap2 opening ceremony state.
pub mod opening;
/// Puzzle pack loading.
pub mod puzzle;
/// Line interpreter.
pub mod repl;
/// Session state machine.
pub mod session;
/// commands that interact with the shell
pub mod shell_command;
/// Terminal UI.
pub mod tui;
