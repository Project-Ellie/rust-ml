# A progress monitor for `nettrain`

**Status:** design proposal, 2026-11-11. Nothing here is locked; the
claim labels `[measured]` / `[derived]` / `[experiment]` /
`[invention]` follow the honesty convention of chapter 12 (see the
glossary).

## Abstract

The `nettrain` binary trains the Gomoku policy/value network for
minutes at a time while printing nothing between its two startup
lines and its final report. This document designs a **progress
monitor** for it: a small *reporter seam* in the training loop that
publishes one event per optimizer step and one per holdout
evaluation, plus three interchangeable consumers of that stream — a
silent reporter for tests, a line reporter for piped output, and a
full-screen terminal monitor for interactive runs. The terminal
monitor shows a thematic progress bar (stones filling a Gomoku
line), live smoothed loss with sparklines for the policy and value
loss components, learning-rate phase, throughput, ETA, holdout
deltas, and a *probe board* panel that renders the network's policy
over fixed tactical positions at every evaluation — the user watches
the network learn to find the winning move. The seam also yields a
`metrics.jsonl` stream written next to the checkpoint, so live
display and durable evidence are the same mechanism. Readers
familiar with chapter 12's training design can skip to §5; the
reporter seam (§5) and the monitor layout (§7) are the core.

## Glossary

- **monitor** — the whole facility described here: the reporter seam
  plus its reporters.
- **reporter** — a consumer of training events. Three are designed
  below (§6): the null reporter, the line reporter, and the board
  reporter.
- **reporter seam** — the trait-and-event boundary inserted between
  the training loop and its observers, so the loop stays testable
  and displays stay replaceable. The same word *seam* is used in
  milestone 2 for the `Evaluator` trait between MCTS and evaluation.
- **board reporter** — the full-screen terminal reporter (§7),
  rendered with crossterm, named for its Gomoku-stone progress bar.
- **probe board** — a fixed position, chosen at run start, on which
  the network's policy is re-evaluated at every holdout evaluation
  and rendered as a board-sized heatmap (§7.4).
- **sparkline** — a small word-sized line chart drawn inline with
  text, using the block characters `▁▂▃▄▅▆▇█`; the term is Tufte's
  (see References).
- **EMA** — exponential moving average; used to smooth the noisy
  per-step loss so the displayed trend is readable.
- **claim labels** — `[measured]` (verified on `main`),
  `[derived]` (follows from cited facts), `[experiment]` (an
  estimate to be validated), `[invention]` (proposed here, not
  yet existing). Convention from
  [chapter 12](../12-gomoku-architecture.md); the four-label variant
  used here matches
  [the phase-0 journey sketch](../specs/2026-11-08-phase-0-training-journey.md).

## 1. Problem

[`nettrain`](../../gomoku/crates/net/src/bin/nettrain.rs) runs a
manual training loop
([`net::train::train`](../../gomoku/crates/net/src/train.rs)): Burn
owns the network, the optimizer, and the batch math; the application
owns data loading, augmentation, the loop, evaluation cadence, and
checkpointing
([chapter 9](../09-toward-alphazero.md)). A real-data run — tens of
thousands of samples, the default 128-channel network, the
pure-Rust Flex CPU device — takes minutes to tens of minutes
`[experiment]`, during which the binary prints exactly two startup
lines (device summary, dataset sizes) and then nothing until the
final report.

The cost is not merely impatience. Three concrete failures follow:

1. **No progress signal.** The total step count is known from
   `--steps` before the loop starts, so an exact percentage and a
   throughput-based ETA are available at zero cost — and are not
   shown.
2. **No health signal.** Training divergence, a stalled data
   pipeline, or a learning rate misconfigured ten-fold all look
   identical from the outside: silence. The user discovers a wasted
   run only at the final report.
3. **No evidence trail between checkpoints.** `StepMetrics` and
   `EvalMetrics` accumulate in memory and only their *last* entries
   reach the
   [`RunJournal`](../../gomoku/crates/net/src/checkpoint.rs). The
   loss trajectory — the primary diagnostic artifact of a run — is
   discarded.

## 2. Requirements

Functional:

- **R1.** Show overall progress (steps done / total, percentage,
  ETA) continuously.
- **R2.** Show the current training loss continuously, smoothed
  enough to read through per-batch noise.
- **R3.** Show holdout evaluation results as they happen, with the
  delta against the previous evaluation.
- **R4.** Never require interaction: the monitor is display-only by
  default and must be safe to ignore entirely.

Non-functional:

- **R5.** Zero measurable cost when disabled, and negligible cost
  when enabled. The expensive part is not rendering — it is the
  per-step `loss.into_scalar()` in
  [`net::train::train`](../../gomoku/crates/net/src/train.rs), which
  forces a GPU-to-CPU device synchronization every step and prevents
  cross-step pipelining. The reporter seam must therefore support a
  *metrics stride* (scalar readback and `Step` event only every k
  steps); the sparkline interpolates. Rendering itself is throttled
  (§5.3).
- **R6.** Determinism untouched: a seeded run must produce the same
  loss trajectory with any reporter, including none. Reporters never
  touch the RNG and never feed back into the loop.
- **R7.** Correct behavior without a terminal: piped output, CI, and
  `--no-default-features` lean builds all get a sensible fallback
  (§6).
- **R8.** House style: crossterm for terminal control (already a
  workspace dependency, used by the `cli` crate's
  [tui module](../../gomoku/crates/cli/src/tui/)); no new heavy UI
  framework; no tokio.

## 3. What the loop already knows

Everything the monitor needs exists in the loop today
`[derived]`, in
[`net::train::train`](../../gomoku/crates/net/src/train.rs):

| Signal | Source | Today |
|---|---|---|
| step index, total steps | loop counter, `config.steps` | local |
| combined loss per step | `loss.into_scalar()` | `StepMetrics.loss` |
| learning rate per step | `scheduler.step().base()` | `StepMetrics.lr` |
| holdout loss, policy top-1, value sign | `evaluate()` | `EvalMetrics` |
| throughput, ETA | a clock at the reporter | absent |
| policy/value loss split | `loss::compute_loss` internals | not exposed |
| probe-board policy | one extra forward pass | absent |

Two small extensions close the gap: loss decomposition (§8) and the
probe board (§7.4). Both are additive; neither changes the training
math.

## 4. Design overview

The design has three parts:

1. **A reporter seam** in `net::train` (§5): the loop publishes
   events; anything may listen.
2. **Three reporters** (§6): null (tests, default in code), line
   (piped/CI), board (interactive terminal, the fancy one).
3. **A durable stream** (§9): `nettrain` always appends the same
   events to `<out>/metrics.jsonl`, so the run directory holds the
   full trajectory, not just its last point.

The seam is the load-bearing decision. Once events exist, the board
reporter is one consumer among equals, the JSONL stream is a
four-line reporter, and future consumers (the milestone-5 self-play
loop, a web dashboard) cost nothing here.

## 5. The reporter seam

### 5.1 Events

```rust
/// One thing that happened during training. All payloads are `Copy`
/// metrics the loop already computes; reporters keep their own clocks.
pub enum TrainEvent {
    /// Emitted once before the first step.
    Started(RunInfo),
    /// Emitted after every optimizer step.
    Step(StepMetrics),
    /// Emitted after every holdout evaluation.
    Eval(EvalReport),
    /// Emitted once after the last step or a stop request.
    Finished(FinishInfo),
}
```

- `RunInfo` carries what the header shows and the journal already
  records: model config, `TrainConfig`, device name, dataset and
  holdout sizes.
- `EvalReport` extends `EvalMetrics` with the probe-board results
  (§7.4); without probe boards it is exactly `EvalMetrics`.
- `FinishInfo` carries the stop reason (`Completed` |
  `StopRequested`) so the journal can record interrupted runs (§7.5).

### 5.2 The trait

```rust
/// A consumer of training events. Implementations must be cheap:
/// `on_event` is called once per optimizer step.
pub trait Reporter {
    fn on_event(&mut self, event: &TrainEvent);
}
```

The training function gains a sibling that takes the reporter, and
the existing function delegates with the null reporter — the public
contract of
[`train`](../../gomoku/crates/net/src/train.rs) and every existing
test keep working unchanged:

```rust
pub fn train_with_reporter(
    model: Model, config: &TrainConfig,
    train_samples: Vec<Sample>, holdout_samples: Vec<Sample>,
    device: &Device, reporter: &mut dyn Reporter,
) -> (Model, Vec<StepMetrics>, Vec<EvalMetrics>);

pub fn train(/* as today */) -> (/* as today */) {
    train_with_reporter(/*…*/, &mut NullReporter)
}
```

Trait objects (`&mut dyn Reporter`) rather than a generic parameter:
one monomorphized loop, reporters composable at runtime, and the
same shape as the milestone-2 `Evaluator` seam.

### 5.3 Throttling lives in the reporter

The loop emits one event per step, unconditionally — the event is a
`Copy` struct of a few floats, cheaper than the optimizer step by
orders of magnitude `[derived]`. Cost discipline therefore lives
entirely in the reporters: the board reporter redraws at most every
100 ms (and always on `Eval` and `Finished`), the line reporter
prints at most one line per second. Tests inject events directly, so
throttle logic is unit-testable without a clock-dependent training
run.

## 6. The three reporters

| Reporter | When | Output |
|---|---|---|
| `NullReporter` | tests, library use | nothing |
| `LineReporter` | stderr is not a terminal, or `--progress line` | one timestamped line per throttled step, one per eval |
| `BoardReporter` | interactive terminal (default), `--progress tui` | the full-screen monitor of §7 |

`nettrain` gains one flag, `--progress auto|tui|line|off`, defaulting
to `auto`: board reporter if stderr is a terminal, else line
reporter. The TUI renders on stderr and takes the alternate screen
there, leaving stdout clean for the final report — piping
`nettrain … | tee log` captures the report, not the animation.

`LineReporter` output stays grep-friendly, mirroring the existing
startup lines:

```text
[00:06:12] step 2843/8000 (35.5%) loss=0.693 ema=0.711 lr=2.41e-4 142 steps/s eta=06:12
[00:06:12] holdout @2800 loss=0.702 top1=41.2% vsign=78.0%
```

## 7. The board reporter

The fancy one. Design goals, in order: readable from across the
room, honest about noise, and *of this project* — a Gomoku training
run should look like one.

### 7.1 Frame layout

```text
 nettrain ─ run 2026-11-11T20:41:07 ─ config default (128ch×10res) ─ device flex ─ seed 0
 data: 45 080 train / 4 920 holdout ─ batch 128 ─ eval every 500

 ●●●●●●●●●●●●●●●●●○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○○  2843/8000   36 %   eta 06:12
 142 steps/s ─ lr 2.41e-4 (cosine, warmup done @800)

 loss   0.693  ema 0.711 ▼   [log]
 policy 0.512  ▁▂▃▅▃▅▄▅▆▆▇█▇▆▅▄▄▃▃▂▂▂▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁
 value  0.181  ▃▄▅▆▅▄▃▃▂▂▂▂▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁

 holdout @2800   loss 0.702 (−0.031)   policy top-1 41.2 % (+2.8)   value sign 78.0 % (+1.1)

 probes @2800      win-in-1 ✓          forced block ✗         quiet
                    · · · · · ·         · · · · · · ·          · ░ ░ · ·
                    · ● ● ● ░ ·         · ● ● ● ○ ░ ·          ░ █ ░ ░ ·
                    · ○ ○ ░ █ ·         · ○ ○ █ · · ·          · ░ ░ · ·
                   top1 i9 ✓ v +0.94   top1 k8 ✗ v −0.12      v +0.03

 [q] stop & checkpoint    [l] log/linear    [p] probes on/off
```

### 7.2 The progress bar is a Gomoku line

The bar is sixty cells wide; completed steps render as black stones
`●`, remaining steps as empty points `○`. A five-in-a-row run wins
Gomoku, and the monitor leans on that: at every multiple of 5 %
progress, the five newest stones flash once. This is pure theme —
the mechanism is an ordinary percentage bar — but it makes the
artifact recognizable, and the flash doubles as a heartbeat when the
ETA is long. In monochrome terminals the bar degrades to `█` and
`░`.

ETA comes from the mean step time over the last 60 seconds of steps
(wall-clock-anchored, not step-count-anchored: 200 steps of history
cover minutes on Flex but seconds on a GPU), displayed as `mm:ss` (or
`hh:mm` when long), and shown as `—` until 10 s of steady-state
history exist. Early steps overstate or understate throughput for two
measured reasons: backend warm-up
([chapter 11](../11-pitfalls.md)) and cubecl autotune re-tuning every
process start (seconds of slow steps before the tuned kernels —
cached on disk — take effect `[measured]` 2026-11-11).

### 7.3 Loss panel

Three rows: combined loss (with EMA and a trend arrow comparing EMA
now vs. 50 steps ago), then one sparkline each for the policy and
value components (§8). Sparklines hold the last ~200 steps, one
column per step, right-aligned to *now*. A `[log]` marker shows the
active scale; `l` toggles linear/logarithmic, because a healthy run
spans more than one decade of loss and a linear sparkline flattens
to a floor within the first percent of training `[experiment]`.

Splitting the sparklines per component is the diagnostically
valuable part: the combined loss can fall while one head stalls, and
a two-row view makes that visible at a glance `[derived]` — the two
components are independent terms of the AlphaZero loss
`(z−v)² − πᵀlog p + c‖θ‖²` (chapter 12).

### 7.4 Probe boards

The signature panel. At run start the reporter picks three fixed
positions from the holdout set, one per datagen label: one *win*
(immediate win exists), one *block* (forced block exists), one
*quiet*. Positions are chosen once, seeded from the run seed, and
kept for the whole run.

At every holdout evaluation, the reporter runs one forward pass per
probe — three forward passes against a holdout sweep of thousands,
negligible `[derived]` — and renders a cropped 6×6 window around the
action for each probe: stones as `●`/`○`, empty cells shaded by
policy mass (the same heat-ramp idea as the study table's visit
heatmap in the `cli` crate), the network's top-1 move marked. Under
each board: `top1` with `✓`/`✗` against the label's known-correct
move, and the value head's output.

This turns training into something watchable: the win-in-1 probe's
heatmap visibly sharpens onto the winning move within the first
evaluations, the block probe follows, the quiet probe stays diffuse.
It is also a genuine health check — if the win-in-1 probe does not
converge to `✓` on a synthetic tactics corpus, something upstream is
broken, and the user sees it *during* the run instead of in a
postmortem `[derived]`.

Feasibility note: `net` already depends on `engine` and `train`, and
`engine::Board::from_position` reconstructs a board from the
absolute move lists a `Sample` stores, so the panel needs no new
dependency and no new board representation `[derived]`.

### 7.5 Controls

Raw-mode keys, all optional — the monitor is complete with zero
input (R4):

- **`q` — stop & checkpoint.** The reporter sets a flag the loop
  checks between steps; on stop, `Finished(StopRequested)` fires and
  `nettrain` saves the checkpoint and journal as usual, with the
  journal recording the interruption (a new `stop_reason` field on
  `RunJournal`, serde-defaulted so old journals still load). Long
  experimental runs become interruptible without losing everything —
  today Ctrl-C discards the run `[measured]`.
- **`l`** — toggle sparkline scale (§7.3).
- **`p`** — toggle the probe panel (small terminals).

Without a terminal, Ctrl-C keeps its current behavior; signal
handling outside raw mode is deliberately out of scope (§10).

## 8. Loss decomposition

`StepMetrics` gains two fields:

```rust
pub struct StepMetrics {
    pub step: usize,
    pub loss: f32,
    /// Policy cross-entropy term, before combination.
    pub policy_loss: f32,
    /// Value mean-squared-error term, before combination.
    pub value_loss: f32,
    pub lr: f64,
}
```

[`net::loss`](../../gomoku/crates/net/src/loss.rs) computes the two
terms on the way to the combined loss already; the change exposes
them. `serde(default)` keeps old journals loadable. The split is
what §7.3 renders and what `metrics.jsonl` records.

## 9. The durable stream: `metrics.jsonl`

Observation and display are separate concerns. `nettrain` therefore
always — regardless of `--progress` — appends every `Step` and
`Eval` event as one JSON object per line to `<out>/metrics.jsonl`,
through a `JsonlReporter` wired next to the display reporter. Cost:
one small serialization per step, buffered `[experiment]`. Payoff:
the run directory (`model.mpk` + `run.json` + `metrics.jsonl`)
carries the full loss trajectory, enough for offline plots in the
paper monograph and for comparing runs after the fact — the phase-0
journey's "what did the run show, where is the evidence" answered by
construction
([journey sketch](../specs/2026-11-08-phase-0-training-journey.md)).

## 10. Alternatives considered

**A single progress bar (indicatif).** The
[indicatif](https://github.com/console-rs/indicatif) crate delivers
R1 in an afternoon, with a battle-tested API. It answers the letter
of the request but not its spirit: loss display means abusing its
message string, there is no canvas for sparklines or probe boards,
and it adds a dependency for 10 % of the goal. Rejected as *the*
solution; acknowledged as the honest minimum if the monitor is ever
de-scoped.

**Burn's own TUI dashboard.** `burn-train` 0.22.0 — the pinned
version, already in the dependency tree — ships a full ratatui-based
metrics dashboard behind its `tui` feature, with line plots,
progress bars, and an interrupter
([renderer module, v0.22.0 tag](https://github.com/tracel-ai/burn/tree/v0.22.0/crates/burn-train/src/renderer)).
Rejected for four reasons `[derived]`: (1) its data model is
Learner-shaped — updates arrive as train/valid `MetricState` with
epoch/iteration progress, so the manual loop would impersonate the
Learner it deliberately does not use (chapter 9's "Burn owns the
network, you own the loop"); (2) the dashboard is generic — no room
for probe boards or the Gomoku theme without forking its widgets;
(3) enabling the `tui` feature pulls ratatui into the `net` crate's
build graph for a dependency the project otherwise avoids; (4) the
project precedent is hand-rolled crossterm (the `cli` crate's
[tui module](../../gomoku/crates/cli/src/tui/)), which keeps one
terminal stack in the workspace instead of two. The seam keeps this
option open: a `BurnTuiReporter` adapter is a future experiment, not
a redesign.

**ratatui from scratch.** More layout power than crossterm alone.
Rejected: a second terminal framework in the workspace against R8,
bought with widgets this design does not need — the frame above is
static text plus two block-character charts.

**Web dashboard / external tooling.** Rejected for scope and
dependencies; `metrics.jsonl` (§9) makes it possible later without
touching the training process.

## 11. Testing

- **Seam**: a `RecordingReporter(Vec<TrainEvent>)` asserts event
  order and counts (`Started`, N×`Step`, one `Eval` per interval,
  `Finished`) on tiny configs.
- **Determinism**: the existing seeded-reproducibility test is
  duplicated with the recording reporter attached; loss trajectories
  must be identical to the null-reporter run (R6).
- **Frames**: the board reporter composes each frame into a string
  grid before touching the terminal; golden-frame tests compare
  against committed expected frames — the same scripted-acceptance
  pattern the `cli` crate uses for its study table `[measured]`.
- **Throttling and ETA**: unit-tested by feeding synthetic event
  sequences with an injected clock.
- **Graceful stop**: a stop-requested run saves a loadable
  checkpoint whose journal records `StopRequested`.

## 12. Consequences

- `net` gains a `report` module (trait, events, null and line
  reporters — dependency-free) and a `report::tui` module behind a
  default-on `tui` cargo feature with an optional crossterm
  dependency; `--no-default-features` yields a lean build, mirroring
  the `cli` crate's `naive-engine` feature pattern `[measured]`.
- `TrainEvent` and the reporters carry no Burn types; the reporter
  seam is backend-agnostic.
- `RunJournal` gains `stop_reason` (serde-defaulted); `StepMetrics`
  gains `policy_loss`/`value_loss` (serde-defaulted).
- The engine dependency island is untouched; nothing here touches
  `engine`, `mcts`, or the locked decisions of chapters 12/13.

## 13. Future work

- **Resume from checkpoint.** The journal comment already says
  "reproduce or resume"; `stop_reason` makes interrupted runs
  identifiable, and the monitor makes interrupting attractive. The
  remaining piece is optimizer-state serialization.
- **The same treatment for `datagen`.** Corpus generation is the
  other long-running binary (50 000 samples in ~45 s debug
  `[measured]`); its quota counter is a natural progress source. A
  shared reporter crate is premature; copy the pattern first,
  extract later.
- **Self-play observability.** Milestone 5's phased loop will run
  for hours; the event-stream-plus-JSONL shape generalizes to it,
  with the board reporter's panels replaced by game-throughput and
  arena-Elo panels.
- **Offline plotting** of `metrics.jsonl` for the paper monograph.

## 14. Measured throughput (2026-11-11, M5 Max)

Benchmarked during the design review of this document; reproduction:
`cargo run --release -p net --features gpu --example layerbench`
(layer attribution) and two-point nettrain timing runs (short vs.
long run per configuration; the slope cancels data loading, model
init, and autotune tuning). All numbers: default config (128 channels
× 10 residual blocks), 50k-sample synthetic corpus, seed 42.

Steady-state cost per step / per 100 samples:

| device | batch | s/step | s / 100 samples |
|---|---|---|---|
| wgpu (no autotune/fusion) | 1024 | 7.98 | 0.78 |
| wgpu + autotune + fusion | 128 | 0.170 | 0.133 |
| wgpu + autotune + fusion | 1024 | 2.48 | 0.242 |
| metal + autotune + fusion + 8 dataloader workers | **128** | **0.137** | **0.107** |
| metal + autotune + fusion + 8 workers | 256 | 0.33 | 0.129 |
| metal + autotune + fusion + 8 workers | 512 | 0.62 | 0.121 |
| metal + autotune + fusion + 8 workers | 1024 | 1.56 | 0.152 |
| metal + autotune + fusion + 8 workers | 2048 | 3.82 | 0.187 |
| flex + 8 workers | any | — | ~1.2–1.4 |

Consequences adopted in `net`: the `gpu` feature enables
`burn/autotune`, `burn/fusion`, and native-MSL `Device::metal`
(`--device metal`); `TrainConfig.dataloader_workers` overlaps CPU
batch encoding with device compute (8 in `nettrain`; 0 keeps seeded
runs bit-reproducible). Diagnosis details: [chapter 11 pitfall
5](../11-pitfalls.md).

Relevance to this design: per-sample throughput is flat from batch
128 to 512, with 128 nominally best (~0.107 s/100 samples, ~12×
flex); below 128 a fixed ~25 ms per-step overhead (loss readback
sync, optimizer step, launch latency) dominates, above 512 per-sample
cost degrades. At that rate the per-step GPU sync of R5 is the
largest remaining reporter-side cost, which is why the metrics stride
is a requirement and not a nicety.

## References

Local:

- [`gomoku/crates/net/src/bin/nettrain.rs`](../../gomoku/crates/net/src/bin/nettrain.rs) — the binary this design instruments
- [`gomoku/crates/net/src/train.rs`](../../gomoku/crates/net/src/train.rs) — the manual training loop; reporter seam goes here
- [`gomoku/crates/net/src/loss.rs`](../../gomoku/crates/net/src/loss.rs) — combined AlphaZero loss; source of the policy/value split
- [`gomoku/crates/net/src/checkpoint.rs`](../../gomoku/crates/net/src/checkpoint.rs) — `RunJournal`; gains `stop_reason`
- [`gomoku/crates/cli/src/tui/`](../../gomoku/crates/cli/src/tui/) — the project's crossterm precedent
- [docs/09-toward-alphazero.md](../09-toward-alphazero.md) — "Burn owns the network, you own the loop"
- [docs/11-pitfalls.md](../11-pitfalls.md) — backend warm-up and parity caveats
- [docs/12-gomoku-architecture.md](../12-gomoku-architecture.md) — loss definition, claim-label convention
- [docs/specs/2026-11-08-phase-0-training-journey.md](../specs/2026-11-08-phase-0-training-journey.md) — runs-as-artifacts discipline this design extends

External:

- burn-train 0.22.0 renderer module (the rejected framework
  dashboard): <https://github.com/tracel-ai/burn/tree/v0.22.0/crates/burn-train/src/renderer>
- indicatif (the rejected minimal option):
  <https://github.com/console-rs/indicatif>
- E. Tufte, *sparklines: intense, simple, word-sized graphics*
  (the term §7.3 uses):
  <https://www.edwardtufte.com/bboard/q-and-a-fetch-msg?msg_id=0001OR>
