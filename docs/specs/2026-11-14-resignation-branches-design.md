# Resignation refutation branches — design

Date: 2026-11-14. Status: approved design, pre-plan.

## Problem

When the AI concedes with `--resign`, the evidence that forced the
concession — one verified opponent VCF certificate per checked
candidate — is only reachable through the typed `decision N` command,
which paints a hypothetical preview on a cloned board. Nothing is
visible at the moment of resignation, and the rejected candidates never
appear in the variation tree.

Wolfie wants: the moment the AI resigns, the relevant information is on
the board, and every rejected candidate is a real, navigable history
branch whose end shows the sequence that refutes it.

This deliberately amends the documented semantic "concession annotates
the current node without adding a stone or changing history"
(cli/README.md). Concession now grows the variation tree. The
annotation itself still adds no stone, advances no cursor, and changes
no engine status.

## Decisions (locked with the owner, 2026-11-14)

1. **Immediate display**: on concession, candidate #1's move +
   refutation paint automatically as a hypothetical overlay; the cursor
   stays at the conceded node. All checked candidates sit as child
   branches, navigable with `forward` / `var N`.
2. **Branch depth**: one child node per rejected candidate (the
   candidate move itself). Entering the branch shows that stone plus
   the opponent's certified VCF as numbered overlay cells. Walking the
   line ply-by-ply is out of scope.
3. **Persistence**: branches and their certificates persist fully in
   saved studies. Branch nodes are ordinary tree nodes; certificates
   re-derive from the parent node's stored audit on load.
4. **`decision` command**: keeps its full textual audit report; the
   typed cloned-board preview entry point is retired (the preview
   renderer survives as the immediate-display machinery). `decision N`
   points at the corresponding branch instead.

## Architecture

`player` is untouched: `resign::consider` already returns
`Vec<RefutedCandidate>` with verified proofs, and admission semantics do
not change.

**`History::ensure_branch(&mut self, mv: Move) -> Result<NodeId, HistoryError>`**
(new): validates legality at the cursor node, returns the existing
child if the move was already explored (human variation, earlier
branch), otherwise inserts a new child. The cursor never moves. Single
validated insertion point, like `play`.

**`SessionCore::record_resignation`** (extend): after appending the
resignation event (existing logic, unchanged), call `ensure_branch` for
every candidate in the evidence, collecting the mapping
`candidate ordinal → variation index`. The emitted `Event::Resigned`
carries this mapping so the UI can announce it.

**Certificates stay single-source**: no proof is copied onto child
nodes. A node is a *refutation branch* iff its parent's latest
audit-bearing resignation event lists the node's incoming move among
its candidates; the stored `Proof` is then rendered from the audit. The
same lookup serves live play and post-load display, so **the v3 record
format does not change**: branch nodes persist as ordinary moves, the
audit already persists, and loading re-derives every branch face.

**Immediate display**: on `Event::Resigned`, the shell sets the
existing preview state (`PreviewSelection { event: latest, candidate:
1 }`) exactly as `:decision 1` does today. The hypothetical board,
numbered cells, and `HYPOTHETICAL PREVIEW; no move played` label are
reused as-is; existing preview-clearing rules apply unchanged.

**Line mode**: concession prints the outcome line plus one line per
candidate (`var 2: h8 — VCF: 1. White i7; 2. Black …`). When the
cursor sits on a branch node, the frame's message line names the
refutation. No paged report is auto-opened in either mode.

## Interaction rules

At the conceded node (active concession):

- Outcome message plus candidate→variation mapping on the message
  line; candidate #1's hypothetical preview painted immediately.
- Existing guards unchanged: Enter/Space/empty line do nothing, typed
  `play`/`step` fail, `take black|white|both` resumes when it includes
  the conceding side.
- `t`/`p` at the conceded node clear the auto-preview exactly as they
  clear a `decision` preview today; `save` and read-only `decision`
  retain it.

At a branch node (cursor entered via `forward` / `var N`):

- The stored certificate paints automatically as numbered cells
  (attacker plain, defender bracketed — the 06d rendering), with a
  caption: `refutation of h8 (event 1, candidate 2)`. It reappears on
  every entry.
- `t` and `p` still work and temporarily replace the certificate
  (same shell-enforced mutual exclusion as 06d); re-entering the node
  restores it. `t` at a branch node is legitimate: it searches the
  side-to-move's forced win and may find a different proof; the stored
  certificate remains labeled as original evidence.
- Playing onward inside a branch is allowed: deeper moves are ordinary
  children with no certificate. The parent's active concession is
  unaffected; only `take` at the conceded node resumes the real game.
- `u`/`back` returns to the conceded node, which shows the concession
  annotation (the auto-preview does not return on `back`).

After `take` resumes play, branches remain as historical variations
with certificates intact, matching the audit's historical/resumed
state.

Legacy v1/v2 records carry no audits: no certificates paint, consistent
with "legacy previews explicitly unavailable".

## Edge cases

- **Fewer than five legal moves**: branches are exactly the checked
  candidates (admission already checks all of them in that case).
- **Candidate move already explored**: `ensure_branch` reuses the
  existing child; the variation index mapping points at it; no
  duplicate node.
- **Multiple concession events at one node** (resume, later
  re-concession): the branch face resolves against the latest
  audit-bearing event; earlier events remain reachable via `decision`.
- **Same move as candidate in two events**: latest event wins, same
  rule.
- **Terminal successor**: impossible — admission already cancels if
  any candidate successor is not ongoing.
- **Legacy load with pre-existing branch-like children**: children
  persist as ordinary moves; without an audit nothing paints.

## Testing

- `history`: `ensure_branch` inserts a validated child without moving
  the cursor; reuses an existing child; rejects illegal moves.
- `session`: concession materializes one branch per candidate with the
  announced variation indices; cursor and engine status unchanged;
  `take`/resume retains branches and certificates; a second concession
  appends independent branches; save/load roundtrip (v3) restores
  branch display derivation without any format change.
- `decision`: report still prints all events/candidates; `decision N`
  prints the branch pointer instead of opening a preview; validation
  and error cases unchanged.
- TUI shell: `Event::Resigned` sets the candidate-1 preview; entering a
  branch paints its certificate; `t`/`p` replace and re-entry restores;
  line mode prints the mapping and per-candidate VCF lines.
- Gates before commit: `cargo test`,
  `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all`.

## Documentation

`gomoku/crates/cli/README.md` is amended: the "without adding a stone
or changing history" concession semantic, the `decision` command table
entries, and the persistence section (branches persist as ordinary
nodes, certificates re-derive). `docs/WARM-UP.md` status snapshot is
updated when the work lands.
