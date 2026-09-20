---
name: scientific-writing
description: Use when writing or editing prose documentation — design docs, primers, tutorials, READMEs, wikis, ADRs, or any expository technical writing. Enforces self-contained documents (abstract first, glossary, define-before-use), plain scientific language without obstructing metaphors or elegant variation, and explicit reference lists. Does not apply to code comments or opt-in solution deep-dives.
---

# Scientific Writing

A quality gate for expository technical prose. The goal is documents a
competent reader can digest **linearly, in isolation, without mental
stumbles** — the standard of a well-written paper, applied to project
documentation.

**Scope:** design docs, primers, tutorials, READMEs, wiki chapters,
ADRs. **Not in scope:** code comments, commit messages, and opt-in
solution deep-dives (e.g., `NN-deep-dive/` folders — those are fine
in their established, more narrative style).

## The Isolation Rule (the hard quality gate)

**Treat every document as if it were written in isolation.** A reader
who has read nothing else in the repository must be able to read the
document front to back without external lookups and without
encountering an undefined term.

Concretely, every document MUST have:

1. **An abstract first.** The document opens with a short section
   (heading `## Abstract`) that states what the document covers, the
   conclusions it reaches, and who should read it — self-contained,
   no forward references. The abstract is the real primer: after
   reading it, the reader knows whether and why to continue.
2. **A glossary immediately after the abstract** (heading
   `## Glossary`) when the document uses any term that is (a) coined
   by the project, (b) colloquial, or (c) standard but used in a
   specialized sense. Symbols used throughout (`π`, `v`, `z`, …)
   are defined there AND again at first use in the body.
3. **Define before use, always.** Never introduce terminology,
   symbols, or abbreviations before defining them. Scan the finished
   document top-down: every non-standard term's first occurrence must
   carry its definition or a pointer to the glossary entry.
   - Exempt: terminology common in mathematical or computer-science
     literature (`argmax`, `softmax`, "tree", "distribution").
   - Not exempt: anything project-coined, anything borrowed from one
     specific paper — cite the original paper for those (e.g., PUCT
     → Rosin 2011).
4. **No assumed knowledge from other documents.** When the document
   depends on material elsewhere: cite the source AND summarize the
   needed content in one or two sentences at the point of use.
   "As shown in ch. 12 §3" is a citation; it is not a substitute for
   the two sentences that let the reader continue without opening
   ch. 12. Summarize — do not duplicate wholesale.
5. **A references section last** (heading `## References`) listing
   every cited source, local and external: repository documents with
   their paths, papers with authors/venue/year, URLs where useful.

## Language Rules

1. **Plain, precise words over imagery.** Metaphors are allowed only
   when they *add* intuition and never when they *replace* the
   mechanism. The test: can the reader state the mechanism after
   reading the sentence? "The network chases the search; the search
   rides the network" fails the test (what is chasing? what is
   riding?) — the mechanism is "training adjusts the network toward
   the search's output; the improved network then produces a stronger
   search." Write that.
2. **No elegant variation.** Referring to the same thing ten times?
   Use the same word ten times. Synonym rotation ("the model" / "the
   network" / "the learner" / "the agent") forces the reader to
   verify identity at every occurrence — that is a prose virtue in
   essays and a defect in scientific writing. Pick one term per
   concept (the glossary fixes it) and hold it.
3. **Avoid colloquial terminology.** Words like "bug farm", "scars",
   "taste", "cherry-pick" (for data) do not belong in the body. When
   a colloquialism genuinely earns its place (it is shorter than any
   precise alternative and used repeatedly), it goes in the glossary
   with a definition, and the body uses it consistently thereafter.
4. **Address the reader neutrally.** "You" is acceptable in
   tutorials; jokes, asides, and personal references
   ("your DeepGomoku scars remember") are not — state the fact:
   "this is the most common defect in MCTS implementations."
5. **Stepping back for the big picture is welcome** — plain-language
   motivation sections are good scientific writing. The rules above
   still apply inside them: plain must remain precise.

## House conventions that interact with this skill

- Claim labels `[paper]` / `[derived]` / `[experiment]` and the
  honesty ledger are project conventions (ch. 12): any document using
  them must define them in its glossary and list ch. 12 in its
  references — the isolation rule applies to conventions too.
- Tutorials keep their established section structure (context →
  intention → …); this skill governs the prose inside it, and every
  tutorial chapter's *Context* section doubles as its
  point-of-use summary of prior chapters (isolation, per-chapter).

## Quality-gate checklist (run before calling a document done)

- [ ] `## Abstract` opens the document; readable standalone.
- [ ] `## Glossary` follows if any coined/colloquial/specialized term
      or recurring symbol exists; every such term is in it.
- [ ] Top-down scan: no term or symbol used before its definition.
- [ ] Every external dependency is cited AND summarized at the point
      of use.
- [ ] `## References` closes the document and is complete (local
      paths + full paper citations).
- [ ] One term per concept, used consistently — grep the synonyms.
- [ ] Every metaphor: does the sentence still state the mechanism?
      If not, rewrite as mechanism.
- [ ] No colloquialism outside the glossary.

## Common mistakes (observed baseline)

| Violation | Fix |
|-----------|-----|
| Symbol `π` used sections before definition ("the improved policy π") | Define at first use + glossary entry: "π: the improved policy — the normalized visit-count distribution over root moves" |
| "The network chases the search; the search rides the network" | "Training adjusts the network toward the search's output; the improved network then produces a stronger search" |
| "fertile bug farm" | "the most common source of implementation defects" |
| "Prerequisites: ch. 12 §1–2" (assumed reading) | Two-sentence summary of the needed content + citation |
| "The honesty ledger applies" (convention undefined in-doc) | Glossary entry + ch. 12 in references |
