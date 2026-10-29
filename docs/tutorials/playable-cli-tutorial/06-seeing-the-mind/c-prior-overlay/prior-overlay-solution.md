# Sub-chapter 06c — The prior view: solution

This file is opt-in. Read it only if you have been stuck for more than
twenty minutes, or after you have finished the sub-chapter and want to
compare your work.

Verified green from `gomoku/`: `cargo test -p cli`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`.

---

The `Prior` variant and `prior_cell` live in `src/tui/overlay.rs`. The
`PolicyOverlay` enum, its cycle, its label, and `overlay_cell` all grow
by one case. The deleted test
`cycle_order_is_off_visits_off` is replaced by the renamed test below.

`gomoku/crates/cli/src/tui/overlay.rs`, lines 22–24:

```rust
    /// Heatmap by prior probability, normalised to the max prior so the
    /// brightest cell is always the move the evaluator liked best.
    Prior,
```

`gomoku/crates/cli/src/tui/overlay.rs`, lines 31–35:

```rust
        match self {
            PolicyOverlay::Off => PolicyOverlay::Visits,
            PolicyOverlay::Visits => PolicyOverlay::Prior,
            PolicyOverlay::Prior => PolicyOverlay::Off,
        }
```

`gomoku/crates/cli/src/tui/overlay.rs`, lines 40–46:

```rust
    pub fn label(self) -> &'static str {
        match self {
            PolicyOverlay::Off => "off",
            PolicyOverlay::Visits => "visits",
            PolicyOverlay::Prior => "prior",
        }
    }
```

`gomoku/crates/cli/src/tui/overlay.rs`, lines 65–71:

```rust
pub fn overlay_cell(info: &SearchInfo, mv: Move, mode: PolicyOverlay) -> Option<OverlayCell> {
    match mode {
        PolicyOverlay::Off => None,
        PolicyOverlay::Visits => visits_cell(info, mv),
        PolicyOverlay::Prior => prior_cell(info, mv),
    }
}
```

`gomoku/crates/cli/src/tui/overlay.rs`, lines 98–125:

```rust
fn prior_cell(info: &SearchInfo, mv: Move) -> Option<OverlayCell> {
    let max_prior = info
        .prior
        .iter()
        .map(|(_, p)| p)
        .fold(0.0f32, |a, &b| a.max(b));
    if max_prior == 0.0 {
        return None;
    }
    let prior = info
        .prior
        .iter()
        .find(|(m, _)| *m == mv)
        .map(|(_, p)| *p)
        .unwrap_or(0.0);
    let intensity = prior / max_prior;
    let is_hot = intensity >= 0.10;
    let text = if is_hot {
        format!("{:.0}%", prior * 100.0)
    } else {
        String::new()
    };
    Some(OverlayCell {
        intensity,
        text,
        is_hot,
    })
}
```

The renamed cycle test and the new prior tests are in
`src/tui/overlay.rs`.

`gomoku/crates/cli/src/tui/overlay.rs`, lines 269–274:

```rust
    #[test]
    fn cycle_order_is_off_visits_prior_off() {
        assert_eq!(PolicyOverlay::Off.cycle(), PolicyOverlay::Visits);
        assert_eq!(PolicyOverlay::Visits.cycle(), PolicyOverlay::Prior);
        assert_eq!(PolicyOverlay::Prior.cycle(), PolicyOverlay::Off);
    }
```

`gomoku/crates/cli/src/tui/overlay.rs`, lines 332–345:

```rust
    #[test]
    fn prior_normalises_by_max_prior() {
        let info =
            search_info_with_visits(&[], &[(mv(7, 7), 0.5), (mv(6, 6), 0.25)], 0.0, Color::Black);
        let best = overlay_cell(&info, mv(7, 7), PolicyOverlay::Prior).unwrap();
        assert_eq!(best.intensity, 1.0);
        assert!(best.is_hot);
        assert_eq!(best.text, "50%");

        let half = overlay_cell(&info, mv(6, 6), PolicyOverlay::Prior).unwrap();
        assert_eq!(half.intensity, 0.5);
        assert!(half.is_hot);
        assert_eq!(half.text, "25%");
    }
```

`gomoku/crates/cli/src/tui/overlay.rs`, lines 347–355:

```rust
    #[test]
    fn below_threshold_prior_is_dim() {
        let info =
            search_info_with_visits(&[], &[(mv(7, 7), 0.5), (mv(6, 6), 0.04)], 0.0, Color::Black);
        let dim = overlay_cell(&info, mv(6, 6), PolicyOverlay::Prior).unwrap();
        assert_eq!(dim.intensity, 0.08);
        assert!(!dim.is_hot);
        assert!(dim.text.is_empty());
    }
```

`gomoku/crates/cli/src/tui/overlay.rs`, lines 357–366:

```rust
    #[test]
    fn a_normalized_prior_of_exactly_ten_percent_is_hot() {
        // The threshold is inclusive, as in the visits path: 0.05 /
        // 0.5 = 0.10 normalized is hot.
        let info =
            search_info_with_visits(&[], &[(mv(7, 7), 0.5), (mv(6, 6), 0.05)], 0.0, Color::Black);
        let boundary = overlay_cell(&info, mv(6, 6), PolicyOverlay::Prior).unwrap();
        assert_eq!(boundary.intensity, 0.1);
        assert!(boundary.is_hot);
    }
```

`gomoku/crates/cli/src/tui/overlay.rs`, lines 368–372:

```rust
    #[test]
    fn empty_prior_list_gives_no_overlay_cells() {
        let info = search_info_with_visits(&[(mv(7, 7), 100)], &[], 0.0, Color::Black);
        assert!(overlay_cell(&info, mv(7, 7), PolicyOverlay::Prior).is_none());
    }
```

`gomoku/crates/cli/src/tui/overlay.rs`, lines 374–378:

```rust
    #[test]
    fn zero_max_prior_gives_no_overlay_cells() {
        let info = search_info_with_visits(&[], &[(mv(7, 7), 0.0)], 0.0, Color::Black);
        assert!(overlay_cell(&info, mv(7, 7), PolicyOverlay::Prior).is_none());
    }
```

`gomoku/crates/cli/src/tui/overlay.rs`, lines 380–387:

```rust
    #[test]
    fn prior_for_missing_move_is_zero_intensity() {
        let info = search_info_with_visits(&[], &[(mv(7, 7), 0.5)], 0.0, Color::Black);
        let cell = overlay_cell(&info, mv(0, 0), PolicyOverlay::Prior).unwrap();
        assert_eq!(cell.intensity, 0.0);
        assert!(!cell.is_hot);
        assert!(cell.text.is_empty());
    }
```

In `src/tui/render.rs`, the policy cell path is generalized so any
active mode passes through, not only `Visits`.

`gomoku/crates/cli/src/tui/render.rs`, lines 259–266:

```rust
    if is_stone || is_cursor || is_last {
        study_cell(board, opening, mv, last, cursor, styled)
    } else if policy_overlay != PolicyOverlay::Off {
        match search_info {
            Some(info) => policy_empty_cell(mv, info, styled, policy_overlay),
            None => study_cell(board, opening, mv, last, cursor, styled),
        }
    } else {
```

`gomoku/crates/cli/src/tui/render.rs`, lines 284–301:

```rust
/// Render an empty cell under a policy overlay.
fn policy_empty_cell(
    mv: Move,
    info: &crate::actor::SearchInfo,
    styled: bool,
    mode: PolicyOverlay,
) -> String {
    let glyph = if HOSHI.contains(&mv.row()) && HOSHI.contains(&mv.col()) {
        '+'
    } else {
        '.'
    };
    match overlay_cell(info, mv, mode) {
        Some(cell) if styled => render_overlay_cell_styled(glyph, &cell),
        Some(cell) => render_overlay_cell_plain(glyph, &cell),
        None => format!(" {} ", glyph),
    }
}
```

The rendering test builds hand-made search info and asserts the prior
percentages, stone precedence, and cursor precedence.

`gomoku/crates/cli/src/tui/render.rs`, lines 1009–1068:

```rust
    #[test]
    fn prior_overlay_shows_percentages_and_respects_precedence() {
        use crate::actor::{AiConfig, SearchInfo};
        use engine::{Color, Move};

        let h8 = Move::new(7, 7).unwrap();
        let i8 = Move::new(7, 8).unwrap();
        let j8 = Move::new(7, 9).unwrap();
        let info = SearchInfo {
            side_to_move: Color::Black,
            visits: vec![],
            prior: vec![(h8, 0.5), (i8, 0.25)],
            value: 0.25,
            config: AiConfig::default(),
        };

        let mut core = SessionCore::new(SessionInit::new_hotseat());
        core.execute(Command::Play { mv: j8 }).unwrap();
        core.set_search_info(info);

        let frame = render_study(&StudyView::new(&core, &[]).policy_overlay(PolicyOverlay::Prior));
        let row7 = &frame[2 + 7];
        let h8_start = 4 + 7 * 3;
        let i8_start = 4 + 8 * 3;
        let j8_start = 4 + 9 * 3;

        // h8 is the evaluator's favourite and shows 50%.
        assert_eq!(
            &row7[h8_start..h8_start + 3],
            "50%",
            "h8 shows the hot prior percentage: {row7}"
        );

        // i8 is warm (50% of max) and also hot; it shows 25%.
        assert_eq!(
            &row7[i8_start..i8_start + 3],
            "25%",
            "i8 shows the warm prior percentage: {row7}"
        );

        // j8 is a stone and the last move, so it keeps its brackets.
        assert_eq!(
            &row7[j8_start..j8_start + 3],
            "[x]",
            "j8 keeps its last-move brackets: {row7}"
        );

        // The cursor on a hot empty cell takes precedence.
        let frame_with_cursor = render_study(
            &StudyView::new(&core, &[])
                .policy_overlay(PolicyOverlay::Prior)
                .cursor(h8),
        );
        let row7_cursor = &frame_with_cursor[2 + 7];
        assert_eq!(
            &row7_cursor[h8_start..h8_start + 3],
            "<+>",
            "the cursor on a hot empty cell keeps the cursor marker: {row7_cursor}"
        );
    }
```

In `src/tui/study.rs` the help text is updated and a loop test checks
two `p` presses.

`gomoku/crates/cli/src/tui/study.rs`, lines 436–439:

```rust
  b, w           Swap2 choice: choose that color
  m              Swap2 first choice: place two more stones (more)
  v              toggle value strip
  p              cycle policy overlay (off → visits → prior)
```

`gomoku/crates/cli/src/tui/study.rs`, lines 866–912:

```rust
    #[test]
    fn two_p_presses_reach_prior_and_show_prior_percentages() {
        use engine::{Color, Move};

        // h7 is used instead of h8 because the cursor starts at h8 and
        // the cursor marker must not be over-painted.
        let h7 = Move::new(6, 7).unwrap();
        let i7 = Move::new(6, 8).unwrap();
        let info = crate::actor::SearchInfo {
            side_to_move: Color::Black,
            visits: vec![],
            prior: vec![(h7, 0.5), (i7, 0.25)],
            value: 0.25,
            config: AiConfig::default(),
        };

        let mut core = SessionCore::new(SessionInit::new_hotseat());
        core.set_search_info(info);

        // Two `p` presses cycle Off → Visits → Prior.
        let keys = vec![char_key('p'), char_key('p'), char_key('q')];
        let (_core, screen) = run_scripted(core, keys).expect("a scripted session runs clean");

        let frame2 = &screen.frames[2];
        let text = frame2.join("\n");
        assert!(
            text.contains("policy overlay: prior"),
            "the message line reports prior mode: {text}"
        );

        // h7 is the evaluator's favourite and shows 50%.
        let row_line = &frame2[2 + h7.row() as usize];
        let h7_start = 4 + h7.col() as usize * 3;
        let cell = &row_line[h7_start..h7_start + 3];
        assert_eq!(
            cell, "50%",
            "the hottest cell h7 shows 50%, got '{cell}' in: {row_line}"
        );

        // i7 shows 25%.
        let i7_start = 4 + i7.col() as usize * 3;
        let cell = &row_line[i7_start..i7_start + 3];
        assert_eq!(
            cell, "25%",
            "the warm cell i7 shows 25%, got '{cell}' in: {row_line}"
        );
    }
```

The typed command help and the byte-exact transcript are updated.

`gomoku/crates/cli/src/repl.rs`, line 35:

```rust
  p, policy        cycle policy overlay (off → visits → prior)
```

`gomoku/crates/cli/tests/line_mode.rs`, line 125:

```rust
  p, policy        cycle policy overlay (off → visits → prior)
```
