//! The line-mode fallback, end to end: scripted lines through the real
//! `LineInput` and the real `study_loop`, with the output captured by a
//! `LineScreen` over an in-memory writer. No terminal is involved.

use cli::session::{SessionCore, SessionInit};
use cli::tui::screen::LineScreen;
use cli::tui::study::{LineInput, study_loop};
use engine::Color;
use std::io::Cursor;

use testkit::{mv, swap2_core};

/// Run the line-mode loop over a script and return the finished core
/// and the captured transcript.
fn run_line_script(mut core: SessionCore, script: &str) -> (SessionCore, String) {
    let mut screen = LineScreen::new(Vec::new());
    let mut input = LineInput::new(Cursor::new(script.to_string()));
    study_loop(&mut core, &mut screen, &mut input, None).expect("a scripted session runs clean");
    let bytes = screen.into_inner();
    (
        core,
        String::from_utf8(bytes).expect("the transcript is UTF-8"),
    )
}

/// The exact transcript of [`the_two_move_transcript_is_byte_exact`],
/// byte for byte: the initial frame, one frame per processed line,
/// the help frame, and the frame after the dismissal.
///
/// WARNING: many lines end in two spaces. The renderer joins the board
/// and the panel with a two-space gutter, and rows past the end of the
/// panel end there. The raw string preserves those spaces; do not
/// strip trailing whitespace in this file.
const TWO_MOVE_TRANSCRIPT: &str = r#"     a  b  c  d  e  f  g  h  i  j  k  l  m  n  o       -- Study table --
   +---------------------------------------------+     Black: Human
 1 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 1   White: Human
 2 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 2   
 3 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 3   Ply: 0
 4 | .  .  .  +  .  .  .  +  .  .  .  +  .  .  . | 4   To move: Black
 5 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 5   Path: (root)
 6 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 6   
 7 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 7   Last events:
 8 | .  .  .  +  .  .  .  +  .  .  .  +  .  .  . | 8     (none)
 9 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 9   
10 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 10  ? help · q quit
11 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 11  
12 | .  .  .  +  .  .  .  +  .  .  .  +  .  .  . | 12  
13 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 13  
14 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 14  
15 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 15  
   +---------------------------------------------+     
     a  b  c  d  e  f  g  h  i  j  k  l  m  n  o       
     a  b  c  d  e  f  g  h  i  j  k  l  m  n  o       -- Study table --
   +---------------------------------------------+     Black: Human
 1 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 1   White: Human
 2 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 2   
 3 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 3   Ply: 1
 4 | .  .  .  +  .  .  .  +  .  .  .  +  .  .  . | 4   To move: White
 5 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 5   Path: h8
 6 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 6   
 7 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 7   Last events:
 8 | .  .  .  +  .  .  . [x] .  .  .  +  .  .  . | 8     Moved h8 by Human
 9 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 9   
10 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 10  ? help · q quit
11 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 11  
12 | .  .  .  +  .  .  .  +  .  .  .  +  .  .  . | 12  
13 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 13  
14 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 14  
15 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 15  
   +---------------------------------------------+     
     a  b  c  d  e  f  g  h  i  j  k  l  m  n  o       
Moved h8 by Human
     a  b  c  d  e  f  g  h  i  j  k  l  m  n  o       -- Study table --
   +---------------------------------------------+     Black: Human
 1 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 1   White: Human
 2 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 2   
 3 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 3   Ply: 2
 4 | .  .  .  +  .  .  .  +  .  .  .  +  .  .  . | 4   To move: Black
 5 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 5   Path: h8 i8
 6 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 6   
 7 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 7   Last events:
 8 | .  .  .  +  .  .  .  x [o] .  .  +  .  .  . | 8     Moved h8 by Human
 9 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 9     Moved i8 by Human
10 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 10  
11 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 11  ? help · q quit
12 | .  .  .  +  .  .  .  +  .  .  .  +  .  .  . | 12  
13 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 13  
14 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 14  
15 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 15  
   +---------------------------------------------+     
     a  b  c  d  e  f  g  h  i  j  k  l  m  n  o       
Moved i8 by Human
     a  b  c  d  e  f  g  h  i  j  k  l  m  n  o       -- Study table --
   +---------------------------------------------+     Black: Human
 1 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 1   White: Human
 2 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 2   
 3 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 3   Ply: 2
 4 | .  .  .  +  .  .  .  +  .  .  .  +  .  .  . | 4   To move: Black
 5 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 5   Path: h8 i8
 6 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 6   
 7 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 7   Last events:
 8 | .  .  .  +  .  .  .  x [o] .  .  +  .  .  . | 8     Moved h8 by Human
 9 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 9     Moved i8 by Human
10 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 10  
11 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 11  ? help · q quit
12 | .  .  .  +  .  .  .  +  .  .  .  +  .  .  . | 12  
13 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 13  
14 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 14  
15 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 15  
   +---------------------------------------------+     
     a  b  c  d  e  f  g  h  i  j  k  l  m  n  o       
Illegal Move Cell is already occupied.
Commands (case-insensitive):
  SQUARE           play a move, e.g. "h8"
  play SQUARE      same as above
  step             let the AI play
  back [N]         go back N plies (default 1)
  forward [N]      go forward N plies (default 1)
  var N            choose child variation N
  take black|white|both      make seat(s) human
  release black|white|both   release seat(s) to AI
  sims black|white N         set per-seat simulations
  info             show AI search info at the cursor
  new              reset to opening
  h, help, ?       show this help
  q, quit, exit    quit

Swap2 opening (sessions started with swap2):
  play SQUARE      place the next stone (colors alternate, black first)
  back [N]         remove the last placement(s)
  done             end the current placement round
  black | white    choose that color at a choice phase
  more             decline the first choice and place two more stones

Press Enter to return to the table.
     a  b  c  d  e  f  g  h  i  j  k  l  m  n  o       -- Study table --
   +---------------------------------------------+     Black: Human
 1 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 1   White: Human
 2 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 2   
 3 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 3   Ply: 2
 4 | .  .  .  +  .  .  .  +  .  .  .  +  .  .  . | 4   To move: Black
 5 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 5   Path: h8 i8
 6 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 6   
 7 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 7   Last events:
 8 | .  .  .  +  .  .  .  x [o] .  .  +  .  .  . | 8     Moved h8 by Human
 9 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 9     Moved i8 by Human
10 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 10  
11 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 11  ? help · q quit
12 | .  .  .  +  .  .  .  +  .  .  .  +  .  .  . | 12  
13 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 13  
14 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 14  
15 | .  .  .  .  .  .  .  .  .  .  .  .  .  .  . | 15  
   +---------------------------------------------+     
     a  b  c  d  e  f  g  h  i  j  k  l  m  n  o       
Illegal Move Cell is already occupied.
"#;

#[test]
fn the_two_move_transcript_is_byte_exact() {
    let core = SessionCore::new(SessionInit::new_hotseat());
    // Two moves, an illegal repeat of the first (the error line), the
    // help screen dismissed with Enter, and quit.
    let (core, transcript) = run_line_script(core, "h8\ni8\nh8\nhelp\n\nquit\n");

    assert_eq!(transcript, TWO_MOVE_TRANSCRIPT);

    // The named properties, asserted on their own so a regression says
    // what it broke before the byte comparison dumps two frames:
    // - no cursor marker anywhere (line mode has no cursor);
    assert!(!transcript.contains('<'), "no cursor marker: {transcript}");
    // - the error line is the session's own error text;
    assert!(
        transcript.contains("Illegal Move Cell is already occupied."),
        "the illegal repeat lands on the message line: {transcript}"
    );
    // - the help screen shows the typed language, not the key table
    //   for keys that do not exist here;
    assert!(
        transcript.contains("Commands (case-insensitive):"),
        "the typed language: {transcript}"
    );
    assert!(
        !transcript.contains("Normal-mode keys"),
        "no key table in line mode: {transcript}"
    );
    assert!(
        transcript.contains("Press Enter to return to the table."),
        "the line-mode dismissal note: {transcript}"
    );
    // - and the stones landed.
    assert_eq!(core.board().stone_at(mv(7, 7)), Some(Color::Black));
    assert_eq!(core.board().stone_at(mv(7, 8)), Some(Color::White));
    assert_eq!(core.snapshot().ply, 2);
}

#[test]
fn the_ceremony_runs_entirely_on_typed_commands() {
    let core = swap2_core();
    // The full protocol: alternated placements, an undo, the
    // count-validated `done`, the declined first choice, and the final
    // choice — every step typed, no keys.
    let script = "play h8\n\
                  play i8\n\
                  back\n\
                  play i8\n\
                  play j8\n\
                  done\n\
                  more\n\
                  play g8\n\
                  play f8\n\
                  done\n\
                  white\n\
                  quit\n";
    let (core, transcript) = run_line_script(core, script);

    // The phases appear in protocol order across the frames.
    let mut at = 0;
    for phase in [
        "Swap2: Placing3",
        "Swap2: FirstChoice",
        "Swap2: Placing2",
        "Swap2: FinalChoice",
    ] {
        let found = transcript[at..]
            .find(phase)
            .unwrap_or_else(|| panic!("{phase} missing after byte {at}: {transcript}"));
        at += found + phase.len();
    }

    // The undo and the finish are facts in the transcript.
    assert!(
        transcript.contains("Removed stone from i8"),
        "the typed back: {transcript}"
    );
    assert!(
        transcript.contains("Opening finished, chose White"),
        "the finish: {transcript}"
    );

    // The finished opening: the ceremony is over, White moves first,
    // and the five stones sit where the script put them, colored by
    // the strict alternation B, W, B, W, B.
    assert!(core.snapshot().opening.is_none(), "the ceremony finished");
    assert_eq!(core.board().to_move(), Color::White);
    assert_eq!(core.board().stone_at(mv(7, 7)), Some(Color::Black)); // h8
    assert_eq!(core.board().stone_at(mv(7, 8)), Some(Color::White)); // i8, re-placed
    assert_eq!(core.board().stone_at(mv(7, 9)), Some(Color::Black)); // j8
    assert_eq!(core.board().stone_at(mv(7, 6)), Some(Color::White)); // g8, round two
    assert_eq!(core.board().stone_at(mv(7, 5)), Some(Color::Black)); // f8, round two
}

#[test]
fn a_color_token_on_play_is_a_parse_error() {
    // The strict alternation makes the token meaningless, so the
    // grammar rejects it; the transcript shows the parser's message.
    let core = swap2_core();
    let (core, transcript) = run_line_script(core, "play h8 white\nquit\n");
    assert!(
        transcript.contains("play takes no arguments, got extra 'white'"),
        "the color token is rejected: {transcript}"
    );
    // The failed line placed nothing: the ceremony is still empty.
    let opening = core.snapshot().opening.expect("the ceremony still runs");
    assert!(opening.placements.is_empty());
}

#[test]
fn eof_quits_the_session_without_a_quit_command() {
    let core = SessionCore::new(SessionInit::new_hotseat());
    // A piped script ends at EOF; the loop must quit, not error.
    let (core, transcript) = run_line_script(core, "h8\n");
    assert!(
        transcript.contains("Moved h8 by Human"),
        "the one scripted move ran: {transcript}"
    );
    assert_eq!(core.board().stone_at(mv(7, 7)), Some(Color::Black));
}
