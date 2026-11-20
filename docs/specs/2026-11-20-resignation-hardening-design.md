# Unified candidate check count

Date: 2026-11-20. Status: implemented on `main`; amended the same day to
unify the guard and resignation counts after the split-cap loss described
below.

## Abstract

The heuristic AI resignation check no longer hardcodes five candidates,
and the defensive guard no longer carries its own candidate cap. One
shared figure — the `PlayerOptions::candidate_checks` field, default
**10**, in
[gomoku/crates/player/src/lib.rs](../../gomoku/crates/player/src/lib.rs)
— now drives BOTH the resignation admission and the defensive guard's
bounded refutation experiment. Everything else about either mechanism —
ranking order, per-candidate probe budget, and fail-fast cancellation —
is unchanged. An earlier draft of this document proposed a two-stage
"hardening" pass; the project rejected it as redundant, because with
identical ranking, budget, and cancellation rules a second stage over
ranks 6..=5+N is exactly a single stage with a larger count. Readers who
touch concession admission, the move guard, the CLI save/load audit
records, or the selfplay provenance records should read this document;
it also records how records written before this change are interpreted.

## Glossary

- **Candidate**: a legal root move admitted to the resignation check or
  the guard's refutation experiment, selected from the front of the
  ranked root edge list.
- **Ranking**: root edges sorted by visit count descending, then pre-noise
  prior descending, then move index ascending.
- **VCF (victory by continuous four)**: a forced win through an
  uninterrupted sequence of four threats. The engine's threat-space
  search produces such proofs and `engine::verify_line` verifies them.
- **Fail-fast**: any candidate that cannot be certified — illegal root
  edge, terminal successor, missing/unknown/exhausted probe, wrong
  winner, or unverifiable proof line — cancels the entire resignation
  immediately.
- **v3 record**: the current CLI save-file format, which persists each
  concession's original evidence (effective options, root statistics, and
  candidate proofs).

## Background

With `--resign` (or `PlayerOptions.resign` set), a real Monte Carlo Tree
Search (MCTS) runs first; then, before noise or sampling, the player
checks the top
`min(candidate_checks, legal_count)` ranked candidates. The
resignation fires only when every checked candidate's successor holds a
verified opponent VCF proof, probed with the baseline guard budget
(1,000 nodes / 10 plies). An own immediate win anywhere on the board
cancels the check. This is a heuristic concession, not an all-legal-moves
loss proof: unchecked lower-ranked moves may still defend. The mechanism
landed with the fixed count 5
([2026-11-07-save-load-and-resignation-solution.md](2026-11-07-save-load-and-resignation-solution.md)).

The defensive guard had its own cap, `GuardConfig::max_candidates`
(baseline 5), while resignation admission checked 10. A real game
analysis (incident 2026-11-20) showed the split losing a game: the guard
probed 5 candidates, all were refuted, and its BudgetExhausted fallback
played the 6th-ranked untested move, which also lost — while genuine
defenders sat at ranks 7 and 10, outside the guard's cap but inside the
resignation count. Two caps over one ranked list serve no purpose: the
guard's smaller cap only manufactured an untested fallback band.

## Decisions

1. **One configuration knob, owned by the player, shared by both
   mechanisms.** `PlayerOptions::candidate_checks: usize`, default
   `player::DEFAULT_CANDIDATE_CHECKS` (= 10), flows from
   `AiPlayer::decide` into `resign::consider` and into `guard::choose`,
   both of which take the count as an explicit parameter.
   `GuardConfig` loses its `max_candidates` field and keeps only
   `enabled` and `budget`; the guard has no count of its own.
2. **Flat single-stage semantics.** Admission is exactly the pre-change
   algorithm with the count substituted: same ranking, same baseline
   guard budget per candidate, same fail-fast cancellation. There is no
   trigger condition, no second phase, and no rank-based diagnostics.
3. **Zero means no candidate checks.** `candidate_checks = 0` checks no
   candidate: no candidate set can be certified, so the concession never
   triggers, and an enabled guard returns its proposal unchecked with a
   BudgetExhausted outcome — the same semantics both mechanisms already
   had for a zero count.
4. **Counts above the legal move count clamp.** With fewer legal moves
   than the count, every legal move is checked — the pre-existing
   endgame behavior, now reachable from any count.
5. **Persistence stores the effective count.** v3 audit records carry
   `candidate_checks` inside the stored options, and import validates
   each concession's candidate count against it
   (`min(count, legal_count)`). Records written before the unification
   stored `resignation_candidates`; the field deserializes under that
   alias. Records predating the configurable count stored neither and
   deserialize with the historical value 5, which is the rule they were
   written under; legacy v1/v2 annotations remain capped at 5. The old
   per-guard `max_candidates` key is accepted on load and ignored — the
   unified count supersedes it. Selfplay seat records likewise store
   `candidate_checks` under the same alias, defaulting to the current
   baseline for older corpora (corpus runs disable resignation, so the
   default is provenance metadata only).

## Rejected alternative

The draft version of this document proposed two-stage hardening: certify
the top 5, then probe N further candidates (ranks 6..=5+N) before
conceding, with a "hardened off" notice when an extra candidate
survived. The project rejected the staging as redundant: with identical
ranking, budget, and fail-fast rules, two stages over 5+N candidates are
indistinguishable from one stage over a configurable count, and the flat
design needs no trigger condition, no second config field, and no new
diagnostics. The observable difference between the draft and this design
is only the cancelled-by-hardening notice, which the flat design drops
along with the staging.

## Testing

- `player`: default is 10; counts 3 and 12 are honored in ranking order;
  count 0 never resigns and never probes; a count above the legal move
  count checks every legal move; the pre-existing admission tests
  (ranking ties, invalid roots, own-win veto, fail-fast on unknown,
  exhausted, malformed, and unverifiable proofs) run unchanged with
  explicit counts. The guard tests pass the count explicitly; zero keeps
  the original proposal unchecked.
- `cli`: save/load round-trips a 10-candidate original audit; records
  with a candidate count disagreeing with the stored options are
  rejected; pre-change v3 records without the field load under the
  historical count 5; pre-unification v3 records with
  `resignation_candidates` and a per-guard `max_candidates` load with
  the unified count.
- Gates before commit: `cargo test`,
  `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all`.

## References

- [gomoku/crates/player/src/lib.rs](../../gomoku/crates/player/src/lib.rs)
  — `PlayerOptions`, `DEFAULT_CANDIDATE_CHECKS`, `AiPlayer::decide`
- [gomoku/crates/player/src/resign.rs](../../gomoku/crates/player/src/resign.rs)
  — the admission check
- [gomoku/crates/player/src/guard.rs](../../gomoku/crates/player/src/guard.rs)
  — the bounded defensive refutation experiment
- [gomoku/crates/cli/src/history/audit_record.rs](../../gomoku/crates/cli/src/history/audit_record.rs)
  — v3 audit persistence and import validation
- [gomoku/crates/cli/README.md](../../gomoku/crates/cli/README.md) — the
  CLI concession contract
- [2026-11-07-save-load-and-resignation-solution.md](2026-11-07-save-load-and-resignation-solution.md)
  — the original fixed-count resignation design
- [2026-11-14-resignation-branches-design.md](2026-11-14-resignation-branches-design.md)
  — branch display for concession candidates; consumes the candidate
  list opaquely and is unaffected by the count
