# Sub-chapter 06d — The TSS overlay + mutual exclusion: solution

This file is opt-in. Read it only if you have been stuck for more than
twenty minutes, or after you have finished the sub-chapter and want to
compare your work.

Verified green from `gomoku/`: `cargo test -p cli`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`.

---

This sub-chapter changes five source files and one integration test.
`overlay.rs` gains the pure TSS mapping and cell renderers. `input.rs`
gains the `t`/`tss` toggle outcome. `render.rs` paints TSS cells with
precedence over the policy overlay. `study.rs` owns the proof state and
runs the prover. `repl.rs` and `tests/line_mode.rs` update their help
texts.

## `gomoku/crates/cli/src/tui/input.rs`

`InputOutcome` gains `ToggleTss` between `CyclePolicy` and `None`.

`gomoku/crates/cli/src/tui/input.rs`, lines 100–103:

```rust
    /// Cycle the policy heatmap overlay on the board.
    CyclePolicy,
    /// Toggle the TSS forced-win overlay on the board.
    ToggleTss,
```

The normal-mode global-key table returns the new outcome for `t`.

`gomoku/crates/cli/src/tui/input.rs`, lines 204–206:

```rust
        KeyCode::Char('v') => return InputOutcome::ToggleValueStrip,
        KeyCode::Char('p') => return InputOutcome::CyclePolicy,
        KeyCode::Char('t') => return InputOutcome::ToggleTss,
```

`line_outcome` recognizes the shell-level words `t` and `tss` before
falling back to the shared command grammar.

`gomoku/crates/cli/src/tui/input.rs`, lines 302–307:

```rust
    if trimmed.eq_ignore_ascii_case("p") || trimmed.eq_ignore_ascii_case("policy") {
        return InputOutcome::CyclePolicy;
    }
    if trimmed.eq_ignore_ascii_case("t") || trimmed.eq_ignore_ascii_case("tss") {
        return InputOutcome::ToggleTss;
    }
```

The two new input tests cover the `t` key and the typed words.

`gomoku/crates/cli/src/tui/input.rs`, lines 777–797:

```rust
    #[test]
    fn t_in_normal_mode_toggles_tss_overlay() {
        let core = playing_core();
        let mut state = InputState::new();
        assert_eq!(
            handle_key(char_key('t'), &mut state, &core),
            InputOutcome::ToggleTss
        );
    }

    #[test]
    fn t_and_tss_in_line_mode_toggle_tss_overlay() {
        let core = playing_core();
        for word in ["t", "T", "tss", "TSS"] {
            assert_eq!(
                line_outcome(word, &core),
                InputOutcome::ToggleTss,
                "{word} toggles the TSS overlay"
            );
        }
    }
```

## `gomoku/crates/cli/src/tui/overlay.rs`

The new TSS section in `overlay.rs` defines the side enum, the overlay
mapping, the verdict, and the plain and styled cell renderers. The
module remains crossterm-free and uses raw SGR escapes for styled output.

`gomoku/crates/cli/src/tui/overlay.rs`, lines 127–226:

```rust
/// Which side a move in a TSS line belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TssSide {
    /// The side that can force the win.
    Attacker,
    /// The opposite side, forced to block.
    Defender,
}

/// A verified TSS proof turned into per-cell render data.
///
/// The overlay numbers cells in the order they appear in the proof line.
/// Attacker moves render plain; defender moves render bracketed.
#[derive(Debug, Clone)]
pub struct TssOverlay<'a> {
    proof: &'a Proof,
    cells: [Option<TssOverlayCell>; 225],
}

#[derive(Debug, Clone, Copy)]
struct TssOverlayCell {
    number: usize,
    side: TssSide,
}

impl<'a> TssOverlay<'a> {
    /// Build an overlay from a verified proof.
    #[must_use]
    pub fn new(proof: &'a Proof) -> Self {
        let mut cells = [None; 225];
        for (i, &mv) in proof.line.iter().enumerate() {
            let side = if i % 2 == 0 {
                TssSide::Attacker
            } else {
                TssSide::Defender
            };
            cells[mv.index()] = Some(TssOverlayCell {
                number: i + 1,
                side,
            });
        }
        Self { proof, cells }
    }

    /// The underlying proof.
    #[must_use]
    pub fn proof(&self) -> &'a Proof {
        self.proof
    }

    /// Look up the overlay data for one cell.
    #[must_use]
    pub fn get(&self, mv: Move) -> Option<(usize, TssSide)> {
        self.cells[mv.index()].map(|c| (c.number, c.side))
    }
}

/// Verdict text for a verified proof overlay.
#[must_use]
pub fn tss_verdict(proof: &Proof) -> String {
    format!(
        "certified forced win for {:?} in {} ply",
        proof.winner,
        proof.line.len()
    )
}

/// Render one plain TSS overlay cell.
///
/// Numbers stay single-digit because the CLI budget caps `max_depth` at 9.
#[must_use]
pub fn render_tss_cell_plain(n: usize, side: TssSide) -> String {
    debug_assert!(n < 10, "single-digit overlay numbers (max_depth = 9)");
    match side {
        TssSide::Attacker => format!(" {n} "),
        TssSide::Defender => format!("[{n}]"),
    }
}

/// Render one styled TSS overlay cell.
#[must_use]
pub fn render_tss_cell_styled(n: usize, side: TssSide, winner: Color) -> String {
    debug_assert!(n < 10, "single-digit overlay numbers (max_depth = 9)");
    let s = n.to_string();
    let color = match side {
        TssSide::Attacker => winner,
        TssSide::Defender => winner.other(),
    };
    // Raw SGR escapes, like the policy renderer above: overlay.rs
    // stays crossterm-free. The winner's side is cyan and bold, the
    // loser yellow — the legacy UI's convention.
    let styled = match color {
        Color::Black => format!("\x1b[1;36m{s}\x1b[0m"),
        Color::White => format!("\x1b[33m{s}\x1b[0m"),
    };
    match side {
        TssSide::Attacker => format!(" {styled} "),
        TssSide::Defender => format!("[{styled}]"),
    }
}
```

The TSS unit tests sit at the end of the module's test block.

`gomoku/crates/cli/src/tui/overlay.rs`, lines 553–605:

```rust
    // -------------------------------------------------------- TSS overlay

    #[test]
    fn tss_overlay_numbers_attacker_and_defender_cells() {
        let proof = Proof {
            winner: Color::Black,
            line: vec![mv(7, 7), mv(6, 7), mv(5, 5)],
        };
        let overlay = TssOverlay::new(&proof);

        let first = overlay.get(mv(7, 7)).unwrap();
        assert_eq!(first, (1, TssSide::Attacker));

        let second = overlay.get(mv(6, 7)).unwrap();
        assert_eq!(second, (2, TssSide::Defender));

        let third = overlay.get(mv(5, 5)).unwrap();
        assert_eq!(third, (3, TssSide::Attacker));

        assert!(overlay.get(mv(0, 0)).is_none());
    }

    #[test]
    fn tss_verdict_names_winner_and_length() {
        let proof = Proof {
            winner: Color::White,
            line: vec![mv(7, 7), mv(6, 7)],
        };
        assert_eq!(
            tss_verdict(&proof),
            "certified forced win for White in 2 ply"
        );
    }

    #[test]
    fn tss_plain_cells_follow_the_old_convention() {
        assert_eq!(render_tss_cell_plain(3, TssSide::Attacker), " 3 ");
        assert_eq!(render_tss_cell_plain(2, TssSide::Defender), "[2]");
    }

    #[test]
    fn tss_styled_attacker_uses_winner_color() {
        let out = render_tss_cell_styled(1, TssSide::Attacker, Color::Black);
        assert!(out.contains('\x1b'));
        assert!(out.contains('1'));
    }

    #[test]
    fn tss_styled_defender_uses_winner_other_color() {
        let out = render_tss_cell_styled(1, TssSide::Defender, Color::Black);
        assert!(out.contains('\x1b'));
        assert!(out.contains('1'));
    }
```

## `gomoku/crates/cli/src/tui/render.rs`

The renderer imports the TSS helpers and `Proof`.

`gomoku/crates/cli/src/tui/render.rs`, lines 19–22:

```rust
use crate::tui::overlay::{
    PolicyOverlay, TssOverlay, overlay_cell, render_overlay_cell_plain, render_overlay_cell_styled,
    render_tss_cell_plain, render_tss_cell_styled, searching_seat_config, value_strip,
};
```

`StudyView` stores the optional proof and initializes it to `None`.

`gomoku/crates/cli/src/tui/render.rs`, lines 59–61:

```rust
    show_value_strip: bool,
    policy_overlay: PolicyOverlay,
    tss_proof: Option<&'a Proof>,
```

`gomoku/crates/cli/src/tui/render.rs`, lines 76–78:

```rust
            show_value_strip: false,
            policy_overlay: PolicyOverlay::Off,
            tss_proof: None,
```

The chainable setter turns the TSS proof on or off.

`gomoku/crates/cli/src/tui/render.rs`, lines 144–149:

```rust
    /// Set the TSS forced-win overlay.
    #[must_use]
    pub fn tss_proof(mut self, proof: Option<&'a Proof>) -> Self {
        self.tss_proof = proof;
        self
    }
```

`render_study` forwards the proof into `study_board_lines`.

`gomoku/crates/cli/src/tui/render.rs`, lines 169–178:

```rust
    let board_lines = study_board_lines(
        &board,
        opening,
        snapshot.last_move,
        view.cursor,
        view.styled,
        view.policy_overlay,
        view.tss_proof,
        search_info,
    );
```

`study_board_lines` builds a `TssOverlay` when a proof is present and
passes it to every cell.

`gomoku/crates/cli/src/tui/render.rs`, lines 211–226:

```rust
#[allow(clippy::too_many_arguments)]
fn study_board_lines(
    board: &Board,
    opening: Option<&OpeningSnapshot>,
    last: Option<Move>,
    cursor: Option<Move>,
    styled: bool,
    policy_overlay: PolicyOverlay,
    tss_proof: Option<&Proof>,
    search_info: Option<&crate::actor::SearchInfo>,
) -> Vec<String> {
    // TSS and policy overlays are mutually exclusive in the shell. The
    // shell enforces it, but the renderer gives TSS precedence if both
    // are somehow active: a deterministic proof overlay outranks a
    // sampled heatmap.
    let tss_overlay = tss_proof.map(TssOverlay::new);
```

`gomoku/crates/cli/src/tui/render.rs`, lines 236–246:

```rust
            let cell = study_or_policy_cell(
                board,
                opening,
                mv,
                last,
                cursor,
                styled,
                policy_overlay,
                tss_overlay.as_ref(),
                search_info,
            );
```

`study_or_policy_cell` checks the TSS overlay first; if it is present,
it delegates to `tss_cell`.

`gomoku/crates/cli/src/tui/render.rs`, lines 262–294:

```rust
#[allow(clippy::too_many_arguments)]
fn study_or_policy_cell(
    board: &Board,
    opening: Option<&OpeningSnapshot>,
    mv: Move,
    last: Option<Move>,
    cursor: Option<Move>,
    styled: bool,
    policy_overlay: PolicyOverlay,
    tss_overlay: Option<&TssOverlay>,
    search_info: Option<&crate::actor::SearchInfo>,
) -> String {
    // TSS and policy are mutually exclusive; if both are active, TSS
    // wins. The shell clears one when the other is activated.
    if let Some(overlay) = tss_overlay {
        return tss_cell(board, opening, mv, last, cursor, styled, overlay);
    }

    let is_stone = stone_at(board, opening, mv).is_some();
    let is_cursor = cursor == Some(mv);
    let is_last = last == Some(mv);

    if is_stone || is_cursor || is_last {
        study_cell(board, opening, mv, last, cursor, styled)
    } else if policy_overlay != PolicyOverlay::Off {
        match search_info {
            Some(info) => policy_empty_cell(mv, info, styled, policy_overlay),
            None => study_cell(board, opening, mv, last, cursor, styled),
        }
    } else {
        study_cell(board, opening, mv, last, cursor, styled)
    }
}
```

`tss_cell` never over-paints stones, the cursor, or the last move, and
renders proof cells as numbered attacker/defender glyphs.

`gomoku/crates/cli/src/tui/render.rs`, lines 296–327:

```rust
/// Render one cell under a TSS proof overlay.
///
/// Stones, the cursor marker, and the last-move marker are never
/// over-painted. Empty cells in the proof line show the move number;
/// attacker moves are plain, defender moves are bracketed.
fn tss_cell(
    board: &Board,
    opening: Option<&OpeningSnapshot>,
    mv: Move,
    last: Option<Move>,
    cursor: Option<Move>,
    styled: bool,
    overlay: &TssOverlay,
) -> String {
    let proof = overlay.proof();
    let is_stone = stone_at(board, opening, mv).is_some();
    let is_cursor = cursor == Some(mv);
    let is_last = last == Some(mv);

    if is_stone || is_cursor || is_last {
        return study_cell(board, opening, mv, last, cursor, styled);
    }

    if let Some((n, side)) = overlay.get(mv) {
        if styled {
            return render_tss_cell_styled(n, side, proof.winner);
        }
        return render_tss_cell_plain(n, side);
    }

    study_cell(board, opening, mv, last, cursor, styled)
}
```

The rendering tests use a VCF puzzle to obtain a real proof from the
engine.

`gomoku/crates/cli/src/tui/render.rs`, lines 1128–1320:

```rust
    // -------------------------------------------------------- TSS overlay

    /// Build the engine's VCF win-in-3 test position and prove it.
    fn vcf_proof() -> engine::Proof {
        let mut b = engine::Board::new();
        let script = [
            (7, 4),
            (7, 3),
            (7, 5),
            (0, 0),
            (7, 6),
            (0, 2),
            (6, 6),
            (0, 4),
            (8, 8),
            (0, 6),
        ];
        for &(r, c) in &script {
            b.play(engine::Move::new(r, c).unwrap()).unwrap();
        }
        engine::prove_forced_win(
            &b,
            engine::Color::Black,
            engine::SearchBudget {
                max_nodes: 2000,
                max_depth: 6,
            },
        )
        .expect("the VCF puzzle is proven")
    }

    fn core_with_vcf_position() -> SessionCore {
        let mut core = SessionCore::new(SessionInit::new_hotseat());
        let script = [
            (7, 4),
            (7, 3),
            (7, 5),
            (0, 0),
            (7, 6),
            (0, 2),
            (6, 6),
            (0, 4),
            (8, 8),
            (0, 6),
        ];
        for &(r, c) in &script {
            core.execute(Command::Play { mv: mv(r, c) }).unwrap();
        }
        core
    }

    #[test]
    fn tss_overlay_paints_numbered_cells() {
        let proof = vcf_proof();
        let core = core_with_vcf_position();
        let frame = render_study(&StudyView::new(&core, &[]).tss_proof(Some(&proof)));
        let row7 = &frame[2 + 7];
        let row5 = &frame[2 + 5];

        // Move 1: attacker plays h8 (7,7), rendered plain.
        let h8_start = 4 + 7 * 3;
        assert_eq!(
            &row7[h8_start..h8_start + 3],
            " 1 ",
            "the attacker cell shows a plain number: {row7}"
        );

        // Move 2: defender blocks at i8 (7,8), rendered bracketed.
        let i8_start = 4 + 8 * 3;
        assert_eq!(
            &row7[i8_start..i8_start + 3],
            "[2]",
            "the defender cell shows a bracketed number: {row7}"
        );

        // Move 3: attacker plays f6 (5,5), rendered plain.
        let f6_start = 4 + 5 * 3;
        assert_eq!(
            &row5[f6_start..f6_start + 3],
            " 3 ",
            "the final attacker cell shows a plain number: {row5}"
        );

        // The verdict line is not part of the rendered frame; the shell
        // puts it on the message line. The board alone carries the overlay.
        let board_text: String = frame.iter().take(19).cloned().collect();
        assert!(!board_text.contains("certified"));
    }

    #[test]
    fn tss_overlay_preserves_stones_and_last_move() {
        let proof = vcf_proof();
        let core = core_with_vcf_position();
        let frame = render_study(&StudyView::new(&core, &[]).tss_proof(Some(&proof)));
        let row7 = &frame[2 + 7];

        // (7,6) is g8, an existing Black stone in the puzzle and not the
        // last move, so it keeps the plain stone glyph.
        let g8_start = 4 + 6 * 3;
        assert_eq!(
            &row7[g8_start..g8_start + 3],
            " x ",
            "an existing stone keeps its glyph: {row7}"
        );

        // The last move is (0,6) = g1 and keeps its brackets.
        let row0 = &frame[2];
        let g1_start = 4 + 6 * 3;
        assert_eq!(
            &row0[g1_start..g1_start + 3],
            "[o]",
            "the last move keeps its brackets: {row0}"
        );
    }

    #[test]
    fn tss_overlay_cursor_takes_precedence_over_numbers() {
        let proof = vcf_proof();
        let core = core_with_vcf_position();
        let h8 = engine::Move::new(7, 7).unwrap();
        let frame = render_study(
            &StudyView::new(&core, &[])
                .tss_proof(Some(&proof))
                .cursor(h8),
        );
        let row7 = &frame[2 + 7];
        let h8_start = 4 + 7 * 3;
        assert_eq!(
            &row7[h8_start..h8_start + 3],
            "<+>",
            "the cursor on a numbered empty cell keeps the cursor marker: {row7}"
        );
    }

    #[test]
    fn tss_and_policy_overlay_mutual_exclusion_prefers_tss() {
        use crate::actor::{AiConfig, SearchInfo};
        use engine::{Color, Move};

        let proof = vcf_proof();
        let mut core = core_with_vcf_position();
        // Plant a policy overlay on the same cells.
        let h8 = Move::new(7, 7).unwrap();
        core.set_search_info(SearchInfo {
            side_to_move: Color::Black,
            visits: vec![(h8, 999)],
            prior: vec![],
            value: 0.0,
            config: AiConfig::default(),
        });

        // Both overlays active: TSS wins.
        let frame = render_study(
            &StudyView::new(&core, &[])
                .policy_overlay(PolicyOverlay::Visits)
                .tss_proof(Some(&proof)),
        );
        let row7 = &frame[2 + 7];
        let h8_start = 4 + 7 * 3;
        assert_eq!(
            &row7[h8_start..h8_start + 3],
            " 1 ",
            "TSS takes precedence over policy: {row7}"
        );
    }

    #[test]
    fn tss_styled_and_unstyled_frames_differ_only_in_escapes() {
        let proof = vcf_proof();
        let core = core_with_vcf_position();
        let styled = render_study(
            &StudyView::new(&core, &[])
                .tss_proof(Some(&proof))
                .styled(true),
        );
        let plain = render_study(
            &StudyView::new(&core, &[])
                .tss_proof(Some(&proof))
                .styled(false),
        );

        assert!(
            styled.iter().any(|l| l.contains('\x1b')),
            "the styled frame has escape sequences"
        );
        assert!(
            plain.iter().all(|l| !l.contains('\x1b')),
            "the plain frame has no escape sequences"
        );

        let stripped: Vec<String> = styled.iter().map(|l| strip_ansi(l)).collect();
        assert_eq!(stripped, plain);
    }
```

## `gomoku/crates/cli/src/tui/study.rs`

`study.rs` imports `SearchBudget` and `Status` from `engine` and
`tss_verdict` from `overlay`.

`gomoku/crates/cli/src/tui/study.rs`, lines 28–33:

```rust
use crate::tui::input::{InputMode, InputOutcome, InputState, handle_key, line_outcome};
use crate::tui::overlay::{PolicyOverlay, tss_verdict};
use crate::tui::render::{StudyView, render_study};
use crate::tui::screen::{AltScreen, LineScreen, Screen};
use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, KeyEventKind};
use engine::{Move, SearchBudget, Status};
```

The loop's `CyclePolicy` arm clears the TSS proof when a policy overlay
is activated, and the new `ToggleTss` arm calls `toggle_tss`.

`gomoku/crates/cli/src/tui/study.rs`, lines 299–316:

```rust
            InputOutcome::CyclePolicy => {
                shell.policy_overlay = shell.policy_overlay.cycle();
                if shell.policy_overlay != PolicyOverlay::Off {
                    // Mutual exclusion: a policy overlay and a TSS proof
                    // overlay cannot be shown at the same time.
                    shell.tss_proof = None;
                }
                message = Some(
                    if shell.policy_overlay != PolicyOverlay::Off && core.search_info().is_none() {
                        "no search here yet".to_string()
                    } else {
                        format!("policy overlay: {}", shell.policy_overlay.label())
                    },
                );
            }
            InputOutcome::ToggleTss => {
                message = Some(toggle_tss(core, &mut shell));
            }
```

`ShellState` now carries the active proof.

`gomoku/crates/cli/src/tui/study.rs`, lines 340–348:

```rust
#[derive(Debug, Default)]
struct ShellState {
    /// Whether the side panel shows the value strip.
    show_value_strip: bool,
    /// The active board policy heatmap overlay.
    policy_overlay: PolicyOverlay,
    /// The active TSS forced-win proof overlay.
    tss_proof: Option<engine::Proof>,
}
```

`build_frame` takes `&ShellState` and forwards `tss_proof.as_ref()` to
the view.

`gomoku/crates/cli/src/tui/study.rs`, lines 358–371:

```rust
fn build_frame(
    core: &SessionCore,
    state: &InputState,
    cursor: Option<Move>,
    events: &[Event],
    message: Option<&str>,
    styled: bool,
    shell: &ShellState,
) -> Vec<String> {
    let view = StudyView::new(core, events)
        .styled(styled)
        .show_value_strip(shell.show_value_strip)
        .policy_overlay(shell.policy_overlay)
        .tss_proof(shell.tss_proof.as_ref());
```

`toggle_tss` hides an existing proof, refuses the Swap2 ceremony and
finished games, runs the prover with a capped budget, verifies the
result, and clears any active policy overlay.

`gomoku/crates/cli/src/tui/study.rs`, lines 386–425:

```rust
/// Toggle the TSS forced-win overlay.
///
/// If a proof is currently shown, hide it. Otherwise run the engine's
/// threat-space search prover on the current board and, on success,
/// store the verified proof and clear any active policy overlay.
fn toggle_tss(core: &SessionCore, shell: &mut ShellState) -> String {
    if shell.tss_proof.is_some() {
        shell.tss_proof = None;
        return "TSS overlay hidden".to_string();
    }

    // The prover searches game positions; ceremony placements are not
    // a game position, so refuse the same way an AI step is refused.
    if core.snapshot().opening.is_some() {
        return "TSS is not available during the Swap2 ceremony".to_string();
    }

    let board = core.board();
    if board.status() != Status::Ongoing {
        return "TSS demo is only available during ongoing games".to_string();
    }

    let budget = SearchBudget {
        max_nodes: 100_000,
        max_depth: 9,
    };
    let side = board.to_move();
    let Some(proof) = engine::prove_forced_win(&board, side, budget) else {
        return "no forced win proven within budget".to_string();
    };

    if !engine::verify_line(&board, &proof) {
        return "internal error: unverifiable proof".to_string();
    }

    let verdict = tss_verdict(&proof);
    shell.tss_proof = Some(proof);
    shell.policy_overlay = PolicyOverlay::Off;
    verdict
}
```

The normal-mode key table lists `t`.

`gomoku/crates/cli/src/tui/study.rs`, lines 490–492:

```rust
  v              toggle value strip
  p              cycle policy overlay (off → visits → prior)
  t              toggle TSS forced-win overlay
```

The loop integration tests use a VCF position to exercise painting,
mutual exclusion, toggling, the empty-result message, and the ceremony
refusal.

`gomoku/crates/cli/src/tui/study.rs`, lines 1023–1208:

```rust
    // -------------------------------------------------------- TSS overlay

    fn vcf_core() -> SessionCore {
        let mut core = SessionCore::new(SessionInit::new_hotseat());
        let script = [
            (7, 4),
            (7, 3),
            (7, 5),
            (0, 0),
            (7, 6),
            (0, 2),
            (6, 6),
            (0, 4),
            (8, 8),
            (0, 6),
        ];
        for &(r, c) in &script {
            core.execute(Command::Play { mv: mv(r, c) }).unwrap();
        }
        core
    }

    #[test]
    fn t_paints_the_forced_win_line_and_verdict() {
        let core = vcf_core();
        // Move the cursor away from h8 so it does not hide the first
        // attacker move; `t` toggles the overlay on; `q` quits.
        let keys = vec![
            key(KeyCode::Down),
            key(KeyCode::Down),
            char_key('t'),
            char_key('q'),
        ];
        let (_core, screen) = run_scripted(core, keys).expect("a scripted session runs clean");

        // Frame 3 follows the `t` key and carries the overlay.
        let frame3 = &screen.frames[3];
        let text = frame3.join("\n");
        assert!(
            text.contains("certified forced win for Black in 3 ply"),
            "the verdict appears on the message line: {text}"
        );

        // Move 1 is on row 8, column h: plain ` 1 `.
        let row7 = &frame3[2 + 7];
        let h8_start = 4 + 7 * 3;
        assert_eq!(
            &row7[h8_start..h8_start + 3],
            " 1 ",
            "h8 shows the first attacker move: {row7}"
        );

        // Move 2 is the defender block at i8: bracketed `[2]`.
        let i8_start = 4 + 8 * 3;
        assert_eq!(
            &row7[i8_start..i8_start + 3],
            "[2]",
            "i8 shows the defender block: {row7}"
        );
    }

    #[test]
    fn t_after_p_clears_the_policy_overlay() {
        use crate::actor::{AiConfig, SearchInfo};
        use engine::Move;

        let mut core = vcf_core();
        let h8 = Move::new(7, 7).unwrap();
        core.set_search_info(SearchInfo {
            side_to_move: Color::Black,
            visits: vec![(h8, 999)],
            prior: vec![],
            value: 0.0,
            config: AiConfig::default(),
        });

        // `p` turns on visits mode; `t` turns on TSS and must clear it.
        let keys = vec![char_key('p'), char_key('t'), char_key('q')];
        let (_core, screen) = run_scripted(core, keys).expect("a scripted session runs clean");

        // Frame 1 has the policy overlay.
        let frame1 = &screen.frames[1];
        assert!(
            frame1.join("\n").contains("policy overlay: visits"),
            "frame 1 shows visits mode"
        );

        // Frame 2 has the TSS overlay and no policy cells.
        let frame2 = &screen.frames[2];
        let text = frame2.join("\n");
        assert!(
            text.contains("certified forced win for Black in 3 ply"),
            "frame 2 shows the TSS verdict: {text}"
        );
        assert!(
            !text.contains("policy overlay:"),
            "frame 2 has no policy overlay message: {text}"
        );
    }

    #[test]
    fn p_after_t_clears_the_tss_overlay() {
        let core = vcf_core();
        // `t` turns on TSS; `p` cycles policy on and must clear TSS.
        let keys = vec![char_key('t'), char_key('p'), char_key('q')];
        let (_core, screen) = run_scripted(core, keys).expect("a scripted session runs clean");

        // Frame 1 has the TSS overlay.
        let frame1 = &screen.frames[1];
        assert!(
            frame1
                .join("\n")
                .contains("certified forced win for Black in 3 ply"),
            "frame 1 shows the TSS verdict"
        );

        // Frame 2 has no TSS numbers: the policy overlay is on but there
        // is no search info, so the board is bare and the message says so.
        let frame2 = &screen.frames[2];
        let text = frame2.join("\n");
        assert!(
            !text.contains("certified forced win"),
            "frame 2 has no TSS verdict: {text}"
        );
        assert!(
            text.contains("no search here yet"),
            "frame 2 explains the empty search: {text}"
        );
    }

    #[test]
    fn t_toggling_twice_removes_the_overlay() {
        let core = vcf_core();
        let keys = vec![char_key('t'), char_key('t'), char_key('q')];
        let (_core, screen) = run_scripted(core, keys).expect("a scripted session runs clean");

        // Frame 1 shows the overlay and verdict.
        let frame1 = &screen.frames[1];
        assert!(
            frame1
                .join("\n")
                .contains("certified forced win for Black in 3 ply"),
            "frame 1 shows the TSS verdict"
        );

        // Frame 2 shows the overlay hidden message and no numbers.
        let frame2 = &screen.frames[2];
        let text = frame2.join("\n");
        assert!(
            text.contains("TSS overlay hidden"),
            "frame 2 reports the overlay is hidden: {text}"
        );
        assert!(
            !text.contains("certified forced win"),
            "frame 2 has no verdict: {text}"
        );
    }

    #[test]
    fn t_with_no_forced_win_shows_the_budget_message() {
        let core = SessionCore::new(SessionInit::new_hotseat());
        // `t` on an empty board finds no forced win within the budget.
        let keys = vec![char_key('t'), char_key('q')];
        let (_core, screen) = run_scripted(core, keys).expect("a scripted session runs clean");

        let frame1 = &screen.frames[1];
        let text = frame1.join("\n");
        assert!(
            text.contains("no forced win proven within budget"),
            "frame 1 explains the empty result: {text}"
        );
    }

    #[test]
    fn t_during_a_ceremony_is_refused() {
        let core = SessionCore::new(SessionInit::new_hotseat().with_swap2(true));
        let keys = vec![char_key('t'), char_key('q')];
        let (_core, screen) = run_scripted(core, keys).expect("a scripted session runs clean");

        let frame1 = &screen.frames[1];
        let text = frame1.join("\n");
        assert!(
            text.contains("TSS is not available during the Swap2 ceremony"),
            "frame 1 refuses the ceremony: {text}"
        );
    }
```

## `gomoku/crates/cli/src/repl.rs`

The line interpreter's help text documents the typed command.

`gomoku/crates/cli/src/repl.rs`, lines 35–36:

```rust
  p, policy        cycle policy overlay (off → visits → prior)
  t, tss           toggle the TSS forced-win overlay
```

## `gomoku/crates/cli/tests/line_mode.rs`

The byte-exact integration transcript grows by the matching help line.

`gomoku/crates/cli/tests/line_mode.rs`, lines 125–126:

```rust
  p, policy        cycle policy overlay (off → visits → prior)
  t, tss           toggle the TSS forced-win overlay
```
