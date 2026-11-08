//! Read-only inspection of original concession evidence.
//!
//! Reports borrow admitted audits; previews replay only the candidate on a
//! cloned board. Neither operation searches, verifies again, or consumes RNG.

use crate::command::CommandError;
use crate::history::ResignationEvent;
use crate::notation::algebraic;
use crate::session::SessionCore;
use engine::{Board, Proof};

/// Validate one-based event/candidate selection against the current node.
pub fn validate(
    core: &SessionCore,
    event: Option<usize>,
    candidate: Option<usize>,
) -> Result<(), CommandError> {
    let events = core.resignation_events();
    if let Some(e) = event {
        checked_index(e, events.len(), "event")?;
    }
    if let Some(n) = candidate {
        let e = event.unwrap_or(events.len());
        checked_index(e, events.len(), "event")?;
        let audit = events[e - 1]
            .audit
            .as_ref()
            .ok_or(CommandError::DecisionUnavailable)?;
        checked_index(n, audit.evidence.candidates.len(), "candidate")?;
    }
    Ok(())
}

fn checked_index(index: usize, count: usize, kind: &'static str) -> Result<(), CommandError> {
    if index == 0 || index > count {
        Err(CommandError::DecisionOutOfBounds { kind, index, count })
    } else {
        Ok(())
    }
}

/// Format all original events, or one selected event, without consulting live settings.
pub fn report(
    core: &SessionCore,
    event: Option<usize>,
    candidate: Option<usize>,
) -> Result<Vec<String>, CommandError> {
    validate(core, event, candidate)?;
    let events = core.resignation_events();
    if events.is_empty() {
        return Ok(vec!["No concession events at the current node.".into()]);
    }
    let selected = event.or_else(|| candidate.map(|_| events.len()));
    let mut lines = vec!["Original concession decisions (stored evidence; no new search)".into()];
    for (i, e) in events.iter().enumerate() {
        if selected.is_some_and(|n| n != i + 1) {
            continue;
        }
        format_event(&mut lines, i + 1, events.len(), e);
    }
    Ok(lines)
}

fn format_event(lines: &mut Vec<String>, number: usize, count: usize, e: &ResignationEvent) {
    lines.push(format!(
        "Event {number}/{count}: {:?} concession; {}",
        e.summary.by,
        if e.resumed {
            "historical/resumed"
        } else {
            "active"
        }
    ));
    lines.push(format!(
        "Heuristic: {} top candidates refuted; other moves unchecked. Not a global loss proof.",
        e.summary.candidates
    ));
    let Some(audit) = &e.audit else {
        lines.push(
            "Original decision log/proofs unavailable (legacy record); cannot preview.".into(),
        );
        return;
    };
    let s = &audit.search;
    let o = &audit.evidence.options;
    lines.push(format!("Original AI config: {:?}", s.config));
    lines.push(format!("Selection configuration: {} (explicit_selection={}); effective temperature/noise are recorded below", if s.config.explicit_selection { "explicit" } else { "automatic/default eligible" }, s.config.explicit_selection));
    lines.push(format!("Effective PlayerOptions: {o:?}"));
    lines.push(format!("Root perspective {:?}; value {}; actual total visits {} vs requested simulations {}; legal root edges {}", s.side_to_move, s.value, s.visits.iter().map(|(_, n)| u64::from(*n)).sum::<u64>(), o.simulations, s.visits.len()));
    lines.push("Pre-noise root provenance: concession checks precede Dirichlet noise and sampling. Probe budgets are CAPS, not measured work or time.".into());
    for c in &audit.evidence.candidates {
        lines.push(format!("Check {}: candidate {} by {:?}; visits {}; prior {} ({}%); probe CAPS nodes {} / depth {}; opponent winner {:?}", c.ordinal, algebraic(c.mv), e.summary.by, c.visits, c.prior, c.prior * 100.0, c.probe_budget.max_nodes, c.probe_budget.max_depth, c.proof.winner));
        if c.proof.line.is_empty() {
            lines.push("No stored proof line; no certificate to display.".into());
        } else {
            lines.push(format!(
                "Verified original VCF: {}",
                c.proof
                    .line
                    .iter()
                    .enumerate()
                    .map(|(i, mv)| format!(
                        "{}. {:?} {}",
                        i + 1,
                        if i % 2 == 0 {
                            c.proof.winner
                        } else {
                            c.proof.winner.other()
                        },
                        algebraic(*mv)
                    ))
                    .collect::<Vec<_>>()
                    .join("; ")
            ));
        }
    }
}

/// A borrowed original opponent certificate and a hypothetical candidate board.
/// The real session's node, path, ply, turn and status are unchanged.
pub struct Preview<'a> {
    /// Owned clone after playing the selected candidate only.
    pub board: Board,
    /// Original stored proof, starting with the opponent's move.
    pub proof: &'a Proof,
    /// Prominent distinction between preview and actual session state.
    pub label: String,
}

/// Prepare a hypothetical view, never invoking a prover or evaluator.
pub fn preview(
    core: &SessionCore,
    event: Option<usize>,
    candidate: usize,
) -> Result<Preview<'_>, CommandError> {
    validate(core, event, Some(candidate))?;
    let events = core.resignation_events();
    let e = &events[event.unwrap_or(events.len()) - 1];
    let c = &e
        .audit
        .as_ref()
        .ok_or(CommandError::DecisionUnavailable)?
        .evidence
        .candidates[candidate - 1];
    let mut board = core.board();
    board.play(c.mv)?;
    Ok(Preview {
        board,
        proof: &c.proof,
        label: format!(
            "HYPOTHETICAL PREVIEW; no move played; candidate {} by {:?}; {:?} VCF (event {}, candidate {})",
            algebraic(c.mv),
            e.summary.by,
            c.proof.winner,
            event.unwrap_or(events.len()),
            candidate
        ),
    })
}

/// Wrap plain report text into bounded pages, including title and footer.
/// ASCII report content uses one display cell per character; Unicode arrows
/// are not emitted by this formatter. Tiny viewports omit title/footer as needed.
#[must_use]
pub fn pages(lines: &[String], width: usize, height: usize) -> Vec<Vec<String>> {
    let width = width.max(1);
    let height = height.max(1);
    let rows = height.saturating_sub(2).max(1);
    let wrap = |text: &str| -> Vec<String> {
        let chars: Vec<_> = text.chars().collect();
        if chars.is_empty() {
            return vec![String::new()];
        }
        chars.chunks(width).map(|s| s.iter().collect()).collect()
    };
    let wrapped: Vec<_> = lines.iter().flat_map(|s| wrap(s)).collect();
    let count = wrapped.len().div_ceil(rows).max(1);
    wrapped
        .chunks(rows)
        .enumerate()
        .map(|(i, chunk)| {
            let mut page = Vec::new();
            if height >= 2 {
                page.push("Decision report".chars().take(width).collect());
            }
            page.extend_from_slice(chunk);
            if height >= 3 {
                page.push(
                    format!(
                        "{}/{} any key: {}",
                        i + 1,
                        count,
                        if i + 1 == count { "dismiss" } else { "next" }
                    )
                    .chars()
                    .take(width)
                    .collect(),
                );
            }
            page
        })
        .collect()
}
