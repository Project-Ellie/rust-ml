# Chapter 12 — The Gomoku Architecture

This document is the design for the AlphaZero-style Gomoku agent. It reads
as a story: it starts with an empty directory and ends with a running
self-improving training system on one Mac Studio (Apple M5 Max, 128 GB
unified memory). No code files yet — but every choice is made, and the
reasoning behind it is written down.

Two chapters (3 and 8) are marked **[collateral]**. They contain history
and literature background. They are not needed to build the system, but
they explain *why* the system looks the way it does. Read them when you
want depth; skip them when you want progress.

Every number in this document carries one of three labels:

- **[paper]** — stated in a cited paper (URL in the appendix).
- **[derived]** — computed or reasoned from cited values.
- **[experiment]** — a free parameter. The papers do not fix it; we choose
  a starting value and tune it by measurement.

This separation is deliberate. During research for this document we found
that several "well-known AlphaZero values" are folklore: neither the
AlphaGo Zero nor the AlphaZero paper publishes a numeric `c_puct` or a
replay-window size. Where that happens, we say so.

---

## 1. Prologue: the machine we are building

The goal is a program that starts knowing only the rules of Gomoku and,
by playing against itself, becomes strong. Concretely, at the end of this
story we can type:

```bash
gomoku selfplay --workers 14          # generate games
gomoku train    --steps 20000         # improve the network
gomoku arena    --games 200           # measure progress (Elo)
gomoku play                           # play against it in the terminal
gomoku run                            # all of the above, forever
```

and the agent gets measurably better every day it runs. The MNIST
curriculum (chapters 1–8) taught us to express networks, data, and
training in Burn. This document adds the parts MNIST does not have: a
game engine, a search algorithm, a system that generates its own training
data, and a parallelism strategy that keeps one GPU fed from eighteen CPU
cores.

The design follows AlphaZero (Silver et al. 2018), with efficiency ideas
from KataGo (Wu 2019) and engineering lessons from ELF OpenGo (Tian et
al. 2019). Where Gomoku differs from Go, the difference is called out.

---

## 2. Four subsystems, one loop

The AlphaZero system decomposes into four subsystems and one loop:

```text
            ┌─────────────────────────────────────────────┐
            │                 THE LOOP                     │
            └─────────────────────────────────────────────┘

  ┌──────────┐   positions   ┌──────────┐   (s, π, z)   ┌──────────┐
  │  ENGINE  │──────────────▶│   MCTS   │──────────────▶│ TRAINING │
  │  rules,  │               │  search  │               │  replay  │
  │  boards  │◀──────────────│          │               │  buffer  │
  └──────────┘  legal moves  └────┬─────┘               └────┬─────┘
                                  │ leaves (s)               │ batches
                                  ▼                          ▼
                            ┌─────────────────────────────────────┐
                            │         NETWORK  fθ(s) = (p, v)     │
                            └─────────────────────────────────────┘
```

- **Engine.** Knows the rules: place a stone, detect five in a row
  (overlines count — freestyle rules), detect a full board, run the
  Swap2 opening protocol. It also detects tactical forcing moves
  (immediate wins, forced blocks, double threats) — rules-level truth in
  pure bit operations, used as a self-play fast path, as the MCTS mock
  evaluator, and as the milestone-3 synthetic-data generator. On top of
  those detections sits a bounded **threat-space search** (TSS): a
  recursive prover over forcing sequences that certifies "this side wins
  by force" and returns the winning line — the extended tactical
  interface whose proven ±1 labels enrich training (milestone 4). Pure
  Rust, no Burn, no GPU. Must be extremely fast — MCTS touches it millions of
  times per second in aggregate.
- **MCTS.** Turns the network's raw opinion (p, v) into a much stronger
  opinion (π, the visit-count distribution) by lookahead search. Owns one
  search tree per game move.
- **Network.** A residual CNN with two heads. The only learned component.
- **Training.** A replay buffer of self-play positions plus a manual
  training loop (chapter 4 of the tutorial, not `SupervisedTraining` —
  our data arrives continuously, from ourselves).

The loop: the current network guides MCTS in self-play games; finished
games enter the buffer; training samples the buffer and improves the
network; the improved network immediately plays better games. AlphaZero
showed this loop needs no gating, no human data, and no game-specific
features beyond the rules [paper — AlphaZero].

**From 5,064 TPUs to one Mac.** AlphaZero generated self-play on 5,000
first-generation TPUs and trained on 64 second-generation TPUs [paper].
We have one GPU. The algorithm does not change; the *proportions* do. ELF
OpenGo's reimplementation is the proof that this works at smaller scale,
and their paper is unusually honest about what matters at that scale:
batched GPU evaluation, asynchronous pipelines, minimum replay sizes, and
stale BatchNorm moments [paper — ELF]. We adopt their lessons; we do not
adopt their scale (2,000 self-play GPUs).

---

## 3. [collateral] A solved game, and why we play it anyway

*Collateral material — history and literature. Skip if you only want the
build.*

**Gomoku is solved — sort of.** In 1993, L. Victor Allis, H. Jaap van den
Herik, and M. P. H. Huntjens published *Go-Moku and Threat-Space Search*
(University of Limburg, Department of Computer Science) and proved that
freestyle Gomoku on 15×15 is a forced win for the first player. The work
became a chapter of Allis's 1994 PhD thesis *Searching for Solutions in
Games and Artificial Intelligence* (Maastricht University, DOI
10.26481/dis.19940923la — bibliography verified; the thesis PDF itself
was not accessible during research, so we cite the result via the
published record). The method, **threat-space search**, exploits the
game's structure: a threat (an open four) forces the defender's reply, so
the attacker searches only threat sequences — a fraction of the full game
tree. Combined with **proof-number search**, this cracked a tree far too
large for brute force.

**Why this does not trivialize our project.** Three reasons:

1. *The balanced game is open.* Freestyle Gomoku's first-player win is
   exactly why competitive play uses opening protocols (Swap, Swap2) or
   restriction rules (Renju: no double-threes, double-fours, or overlines
   for Black — verified at renju.net). As of this writing, Gomoku under
   the Swap2 opening rule has **not** been solved, and the theoretical
   value of all legal freestyle positions is unknown — only the empty-board
   value is. A learning agent that plays strong Swap2 Gomoku operates on
   real open ground.
2. *We are not building a solver.* Threat-space search is a hand-crafted
   exploit of Gomoku's forcing structure. AlphaZero is the opposite: a
   general method that discovers structure. The scientific interest is in
   the generality — the same code should play any two-player perfect-
   information board game after changing the engine crate.
3. *It gives us a free oracle — twice.* Solved positions and
   threat-sequence puzzles are perfect test data: an agent that cannot
   win a won position in one threat sequence has a bug, and we can test
   for that before any learning happens. And the search itself, run
   bounded and trusted only where it *proves* a forced win, emits exact
   labels — policy = the forcing move, value = rules-truth ±1. That is
   the anchor set milestone 4 adds to training (§13).

**What the Gomoku AI literature adds.** AlphaGomoku (Xie, Fu, Yu 2018,
arXiv:1809.10595) is the closest prior work: AlphaGo-style MCTS for
freestyle 15×15 Gomoku with a *small* network (32 filters, two residual
blocks, a 3-plane input) and a curriculum — first imitation of synthetic
attack/defense moves, then imitation of a rule-based mentor, then
self-play reinforcement. It reports reaching human playing level in two
days on one GPU (prose claim; no full match statistics given). Two
lessons: (a) Gomoku needs far less network than Go — encouraging for our
hardware budget; (b) a curriculum can substitute for compute early. We
do not go fully tabula rasa: the engine carries a small hand-woven
tactics module (win-in-1, forced block, win-in-2 detection — pure bit
operations, no mentor network). It gives the learning curve a head
start, doubles as the mock evaluator for the MCTS milestone, and
generates milestone 3's synthetic attack/defense set: AlphaGomoku's
curriculum idea, with a mentor that is a few dozen lines of bit math we
would build anyway for testing.

---

## 4. Hardware: what an M5 Max actually is

Verified from Apple's August 2026 Mac Studio announcement and
specification pages:

| Component | Specification |
|---|---|
| CPU | 18 cores: 6 "super cores" + 12 "performance cores" (Apple's labels — note: no efficiency cores in this part) |
| GPU | 40 cores (top M5 Max configuration) |
| Memory | 128 GB unified, 614 GB/s bandwidth |
| Neural Engine | 16 cores (Apple publishes no TOPS figure) |
| GPU fp16 throughput | **not published** — do not plan around a number Apple did not release |

Three architectural facts matter more than the numbers:

**Unified memory is real but not magic.** CPU and GPU share one physical
pool. There is no PCIe transfer of the replay buffer or of network
weights — the classic bottleneck of PC self-play rigs. But wgpu still
manages its own buffers and command queues, so "zero-copy" is a property
of memory capacity and bandwidth, not of API absence. What unified memory
buys us concretely: the replay buffer can grow to tens of gigabytes
without any design change, and weight updates never cross a bus.

**The parallelism budget.** 18 CPU cores, all of them fast. MCTS and the
game engine are pure CPU work; network evaluation and training are GPU
work. The entire strategy of chapter 9 is: keep ~14 cores busy searching,
one thread collecting their evaluation requests into GPU batches, one
thread training, and never let any of them wait on a lock longer than a
microsecond.

**The GPU is shared and single.** Self-play evaluation and training both
want the GPU. On a single device we cannot pretend to be DeepMind's
asynchronous TPU fleet without thinking; chapter 9 phases the contention
honestly instead of hoping the driver sorts it out.

---

## 5. Technology choices: the stable edge of the moving frontier

The rule: newest version that is **stable**, where "stable" means a
non-prerelease release with a working ecosystem — not `main`, not a
release candidate, not an abandoned crate.

| Choice | Version | Why this one | What we rejected, and why |
|---|---|---|---|
| Rust | 1.94, edition 2024 | Edition 2024 is stable since 1.85 (Feb 2025); let-chains and improved lifetime capture are genuinely useful in engine code | edition 2021 — no reason to stay behind |
| Burn | **0.21.0, pinned** | Latest stable (2026-05-07). Our MNIST code and the whole tutorial are verified against it | 0.22-pre — removes the backend type parameter from `Tensor`/`Module`; half the ecosystem examples would be wrong |
| GPU backend | `burn::backend::Wgpu` (Metal underneath) | The only Burn path to the M5 Max GPU. Verified in source: `Metal` and `Vulkan` are plain `Wgpu` aliases; CubeCL 0.10 + wgpu 29 underneath; IntElem i32 — same as Flex, so no dtype friction between CPU and GPU code | `cpu` (CubeCL CPU) — immature for training; LibTorch/Candle — deprecated in Burn |
| CPU backend | `burn::backend::Flex` | Parity-test twin and fallback. Pure Rust, deterministic, already proven by our MNIST baseline | NdArray — legacy, the Burn project points new work to Flex |
| Concurrency | `std::thread` + `crossbeam-channel` 0.5 | CPU-bound actors + bounded MPMC channels. `crossbeam-channel` gives us `select!`, which the evaluator's batch-collection loop needs for its timeout | **tokio — rejected, reasoning below**; rayon — fork-join parallelism is the wrong shape for long-running actors (kept in the toolbox for engine batch operations) |
| Serialization | `bincode` 2 + `serde` | Stable 2.x, trivial with `#[derive(Serialize, Deserialize)]`, compact; used by Burn itself | rkyv — zero-copy is attractive for the replay buffer, but its `Archive` derive infects every struct with archive types; the buffer is not our bottleneck, simplicity wins |
| CLI | `clap` 4 derive | Standard | — |
| RNG | `rand` 0.9, `SmallRng` per worker | One seeded RNG per worker thread: no shared-state contention, reproducible streams per worker | one global `thread_rng` — lock contention under 14 workers is real |
| Errors | `thiserror` in library crates, `anyhow` in the `cli` binary | The standard split; typed errors where callers branch on them, ergonomic propagation at the top | — |
| Logging | `tracing` + `tracing-subscriber` | Structured, per-actor spans; we will want to answer "what was worker 7 doing" at 3 AM | `log` — fine, but spans earn their keep in a multi-actor system |

**Why no tokio — the full reasoning.** Async Rust earns its complexity
when a program juggles many *I/O-bound* waits: sockets, files, timers.
Our system is *CPU-bound*: a self-play worker computing MCTS has nothing
to await — it either computes or blocks on one channel send. An async
runtime would add a scheduler, `Pin` pollution through every call chain,
and the constant hazard of blocking the executor with CPU work, in
exchange for nothing we need. OS threads, by contrast, are exactly right:
the kernel preempts them fairly across 18 cores, each worker owns its
stack and its RNG, and `crossbeam-channel` provides the two primitives we
actually need (bounded MPMC queues for backpressure, `select!` for the
evaluator's batch window). This is also the honest reading of Burn's own
examples: the official MNIST selects backends with `std::thread`-era
synchronous code. Async would be résumé-driven design here.

**The Metal risk, stated plainly.** Burn-on-Metal works — our tutorial's
inference would run on it today — but the issue tracker contains a
deterministic report of all-NaN gradients during Wgpu/Metal *training* on
Apple silicon (issue #5162, Burn 0.21.0, M4 Pro) and an autotune-related
wrong-kernel report for 1×1 convolutions (issue #5626). Both patterns —
backward passes and 1×1 convs — are load-bearing in our network
(1×1 convs are both heads). Therefore: **GPU training is gated on a
parity test** (chapter 11). If parity fails, the fallback is training on
Flex/CPU and evaluating on Wgpu — slower, but the system degrades
gracefully instead of silently learning garbage. This is the price of the
bleeding-but-stable edge, and it is cheap.

---

## 6. The workspace: crates as architecture

One Cargo workspace, seven crates. The boundaries are the architecture:
each crate compiles against the *traits* of its dependencies, never their
internals, and `cargo check -p engine` must pass with Burn nowhere in
sight.

```text
gomoku/
├── Cargo.toml            # workspace, pinned burn = "=0.21.0"
├── crates/
│   ├── engine/           # rules, board, moves, win detection, symmetries,
│   │                     # tactics, Swap2 opening, board→planes encoding
│   │                     # deps: none beyond std + serde + thiserror
│   ├── net/              # Burn model, plane→tensor conversion, records
│   │                     # deps: burn, engine
│   ├── mcts/             # PUCT search tree; generic over an Evaluator trait
│   │                     # deps: engine (NOT net — see below)
│   ├── selfplay/         # worker actors, evaluator service, game runner
│   │                     # deps: engine, mcts, net, crossbeam-channel
│   ├── train/            # replay buffer, sampler, manual training loop
│   │                     # deps: burn, net, engine, bincode
│   ├── arena/            # agent-vs-agent matches, Elo, gating
│   │                     # deps: engine, mcts, net
│   └── cli/              # the `gomoku` binary: selfplay|train|arena|play|run
│                         # deps: all + clap + anyhow + tracing
└── docs/                 # this document, ADRs as they appear
```

Three of these boundaries deserve the Rust-specific reasoning:

**`mcts` must not depend on `net`.** The search needs to ask "what is
fθ(s)?" — nothing else. That is a trait:

```rust
// in mcts — the ONLY coupling between search and network
pub trait Evaluator {
    /// Evaluate one position: policy priors over legal moves + value.
    fn evaluate(&mut self, req: EvalRequest) -> EvalResult;
}
```

A blocking-channel client implements it for self-play, a direct
`net::Model` wrapper implements it for tests and arena play, and a mock
implements it for MCTS unit tests. Why a trait and not a generic
parameter `E: Fn(Board) -> (Policy, Value)`? Because the evaluator in
production is a stateful actor client (it holds a channel and a pending-
request table), and `Fn` closures capture awkwardly around that. A named
trait with one method is the Rust Design Patterns answer to "complex
bounds that would repeat everywhere" — and it keeps `mcts` testable with
a two-line mock.

**`engine` is a dependency island.** No Burn, no channels, no I/O. This
is not purism: the engine runs on all 14 worker threads simultaneously,
inside the hottest loop in the system. An island crate is trivially
`Send + Sync` everywhere, compiles in seconds (fast test iteration on the
part of the system with the most property tests), and can never
accidentally acquire a GPU dependency at 2 AM.

**Visibility as enforcement.** Each crate exposes a small `lib.rs`
surface; internals stay `pub(crate)`. Rust gives us this for free, and in
a one-person project it is tempting to make everything `pub` "for now" —
resist that here, because the boundaries in this system are doing real
work: they are what let us swap the network size, the evaluator
transport, or the training schedule without a cross-cutting refactor.

---

## 7. The engine: bits, shifts, and symmetries

Everything in this chapter is `[paper]`-free territory — pure
engineering, where Rust's zero-cost abstractions carry the load.

### Board representation

A 15×15 board has 225 cells. We store it as **two bitboards in absolute
colors** — black and white, plus a side-to-move flag. Absolute storage
is required because our opening protocol, Swap2, places three
non-alternating stones (2 black + 1 white) before colors are chosen; a
relative me/you store cannot express that. The relative view — the
network never sees "black" and "white", it sees "me" and "you", which
halves the state space the policy must learn — is derived at encode
time:

```rust
pub struct Board {
    black: Bitboard,     // absolute colors — Swap2's non-alternating
    white: Bitboard,     //   opening cannot be stored relatively
    to_move: Color,
    moves: Vec<u8>,      // move history, for encoding + games records
    key: u64,            // Zobrist hash, incremental
}

struct Bitboard([u64; 4]);   // 240 bits used, stride 16
```

Layout choice, with reasoning: naive stride-15 packing (cell `r*15+c`)
makes win-detection shifts awkward — horizontal neighbors differ by 1,
vertical by 15, diagonals by 14 and 16, and the three different strides
force masking gymnastics at the row edges. The classic bitboard trick
(chess engines have used it for decades) is **stride 16**: cell `r*16+c`,
one padding column per row. Now the four directions are uniform shifts —
1, 16, 15, 17 — and the padding column absorbs horizontal wrap-around.
240 bits fit in `[u64; 4]` with room to spare.

There are two different edge problems, and they want different answers:

- **Rule-side (compute).** Bit shifts must not wrap around row ends.
  Stride 16 solves this: the padding column absorbs wrap-arounds.
  Invisible to the network.
- **Learning-side (representation).** A 15×15 plane with same-padding
  convolutions pads with *zeros* at the border, so the network sees
  "emptiness" beyond the edge and must learn edge behavior separately
  from center behavior. But the edge is semantically not empty: a wall
  blocks a line exactly like an opponent stone does.

The learning-side answer is the azrust design: the encoder emits **17×17
planes with the border ring set as opponent stones** (chapter 10), so
convolutions see the wall everywhere and a line near the edge "looks"
correctly constrained. Engine internals stay stride-16; the 17×17 border
is purely an encoding-layer concern. The two never mix.

`Board` is a small, `Clone`-cheap value type. No `Box`, no `Rc`, no
interior mutability. MCTS needs one board state per simulated move; the
engine offers both cheap `Clone` (32-ish bytes of plain data plus a
small `Vec` — the difference between cache-resident and allocation-bound
at millions of simulations per second aggregate) and `play`/`undo`.
MCTS decides which by measurement. (If profiling later shows the
`Vec<u8>` move history hurts, it becomes a fixed `[u8; 225]` + length —
a one-line change behind `engine`'s visibility wall.)

### Win detection without loops

Five in a row, direction shift `s`: a stone, and a stone at +s, +2s, +3s,
+4s. On a bitboard that is:

```rust
const DIRS: [u32; 4] = [1, 16, 15, 17]; // horizontal, vertical, two diagonals

fn has_five_dir(b: &Bitboard, s: u32) -> bool {
    let two  = *b & b.shr(s);          // runs of >= 2
    let four = two & two.shr(2 * s);   // runs of >= 4
    let five = four & four.shr(s);     // runs of >= 5
    !five.is_zero()
}
```

Two invariants make this exact. First, **padding bits are always zero**:
every wrap path crosses a padding bit and the AND-chain dies, so no
per-direction edge masks are needed at all — masks are required only
after complement operations (`!occupied` sets padding bits, so
complements immediately AND with a `VALID` mask). Second, the staged
two/four/five AND keeps every shift under 64 (the naive five-term AND
needs 4·17 = 68) and detects *five or more* in one pass — the freestyle
rule we play: **overlines count as a win**.

Four shift-and trees per move instead of scanning the board. Win
detection, move legality (`(black | white)` bit test), and draw
detection (225 moves played) are all O(1)-ish bit operations. The engine's test
suite (chapter 11) will hammer these with property tests against a naive
reference implementation — the classic trick of testing fast code against
obviously-correct slow code.

### Symmetry: eight games for the price of one

The square board has the dihedral symmetry group D4: identity, three
rotations, and their mirrors — 8 transforms. Every position has 8
equivalent forms with identical value and identically-transformed policy.
We implement transforms as **precomputed index permutation tables**
(`[[u8; 225]; 8]`, built once), used for training-data augmentation:
when the sampler draws a position, it applies one random transform to the
planes *and* the policy target π. Transforms operate in the 15×15 space;
the 17×17 border ring is added afterwards, and since the ring is itself
D4-invariant, encoding commutes with every transform exactly. This is
AlphaZero's exact usage
[paper — AlphaZero bakes symmetries in as augmentation, not as network
architecture]. We deliberately do *not* average network evaluations over
transforms at search time (AlphaGo Zero did that in evaluation only);
it costs 8× evaluation for a marginal gain our scale cannot afford.

### Zobrist keys

One `u64` per (cell, color) pair, XORed incrementally per move. The
table is generated by a `const fn` (fixed-seed xorshift) at compile
time — reproducible forever, and no `rand` dependency in the engine.
Purpose: deduplication in the replay buffer and cheap position identity
in tests. We explicitly do **not** merge transpositions inside MCTS — the
tree stays a tree, as in AlphaZero. Transposition-table MCTS exists in
the literature (the MCTS review catalogs it) but interacts subtly with
PUCT statistics; not now.

---

## 8. [collateral] How parallel MCTS actually works

*Collateral material — literature background for chapter 9's decisions.*

The MCTS review (Świechowski et al., arXiv:2103.04931) sorts parallel
MCTS into three families:

- **Leaf parallelization** (Cazenave & Jouandeau, 2007): run many
  playouts from one expanded leaf in parallel. Simple, but it wastes work
  when playouts are correlated, and with a *network* instead of random
  rollouts there is no playout to parallelize — evaluation is already
  batched.
- **Root parallelization** (Chaslot et al., 2008): build independent
  trees, merge root statistics. Zero synchronization, but each tree
  repeats the others' exploration — wasteful, and awkward with a shared
  GPU evaluator.
- **Tree parallelization** (Cazenave & Jouandeau, 2008): one shared
  tree, many threads. The problem: concurrent threads select the same
  "best" path and pile onto one leaf. The fix is the **virtual loss** —
  a thread descending a path temporarily worsens its value, deterring
  followers until the real result is backed up. AlphaGo (2016) ran
  exactly this: asynchronous policy/value MCTS (APV-MCTS), lock-free tree
  updates (Enzenberger & Müller, 2009), virtual loss (citing Segal's
  scalability study), CPU search threads with GPU evaluation queues.

The historical point worth internalizing: **AlphaGo needed tree
parallelization because its network evaluations were scarce and its
search deep.** Our situation is inverted. Evaluations come from one
shared GPU in large batches; what we lack is not threads per tree but
*trees*. Game-level parallelism — each worker plays its own game with its
own single-threaded tree — produces hundreds of independent trees per
minute, keeps every worker's data structures lock-free and
cache-private, and turns the evaluator into the only point of
synchronization (where synchronization is *desirable*: it is the batching
point). ELF OpenGo made the same choice at datacenter scale: 32
self-play workers per GPU, 8 game threads per worker, no shared trees
[paper — ELF]. Tree parallelization with virtual loss stays on the shelf,
documented, ready if single-game latency ever matters more than aggregate
throughput (it will not, in self-play).

---

## 9. The parallelism strategy

Now the engineering heart of the document. Goal: all silicon busy, all
the time, with correctness we can test.

### The actor layout

```text
 ┌─ worker 0 ─┐  ┌─ worker 1 ─┐        ┌─ worker 13 ─┐
 │ game + MCTS│  │ game + MCTS│  ...   │ game + MCTS │   14 threads,
 └─────┬──────┘  └─────┬──────┘        └──────┬──────┘   CPU-bound
       │ EvalRequest   │                      │           (own Board, RNG, tree)
       ▼               ▼                      ▼
 ┌────────────────────────────────────────────────────┐
 │ EVALUATOR SERVICE  (1 thread, owns the GPU model)  │
 │   collect requests ── batch (≤128, ≤2 ms) ──▶ GPU  │
 │   split results ──▶ oneshot reply per request      │
 └───────────────────────┬────────────────────────────┘
                         │ (phased with training, see below)
 ┌───────────────────────▼────────────────────────────┐
 │ TRAINER  (1 thread)                                │
 │   sample buffer ──▶ batch ──▶ fwd+bwd ──▶ step     │
 └───────────────────────┬────────────────────────────┘
                         │ checkpoints
 ┌───────────────────────▼────────────────────────────┐
 │ ARENA (on demand)  candidate vs baseline → Elo     │
 └────────────────────────────────────────────────────┘

 REPLAY BUFFER: shared append + random-sample; see "the buffer" below.
```

### Why 14 workers

18 cores. The evaluator thread is mostly idle-waiting on the GPU; the
trainer alternates phases with self-play (below) and needs CPU headroom
for batch assembly; the OS and our own shell deserve a core. 14 workers
leave ~2 cores of slack — a starting value, tuned by watching one number:
evaluator queue depth. Queue starving → add workers; queue flooded →
remove workers. [experiment]

### The evaluator service: the only synchronizer

Every MCTS leaf evaluation, from every worker, funnels through one
bounded MPMC channel (`crossbeam-channel`, capacity ~4,096
[experiment]). The service loop:

```rust
loop {
    let first = rx.recv()?;                       // block for work
    batch.push(first);
    // fill the batch: more requests, up to limits
    while batch.len() < 128 {
        match rx.recv_timeout(2.ms()) {           // select! in practice
            Ok(req) => batch.push(req),
            Err(_) => break,                      // window closed: run
        }
    }
    let results = model.forward(batch.encode());  // ONE GPU call
    for (req, res) in batch.drain().zip(results) {
        let _ = req.reply.send(res);              // oneshot per request
    }
}
```

Design reasoning, point by point:

- **Dynamic batching** is the whole game. GPU efficiency lives between
  "batch 1, always ready" and "batch 1024, always late". A max batch of
  128 with a 2 ms window is the classic compromise (ELF capped GPU
  inference batches at 128 on V100s for the same latency reasons
  [paper — ELF]); both knobs are `[experiment]`, tuned against queue
  depth and games/hour.
- **One model, one thread, no locks.** The network lives exclusively on
  the evaluator thread. Workers never touch Burn types; they send plain
  `EvalRequest` structs (encoded planes + legal-move mask) and receive
  plain `EvalResult`s. This sidesteps every hard question about `Send`
  on GPU tensor handles, and it means a model swap (after training) is
  one atomic assignment on one thread — no reader-writer lock on the
  weights, ever. In Rust terms: we use *ownership* to make an entire
  class of concurrency bugs unrepresentable, instead of `Arc<Mutex<_>>`
  to manage them. This is the single most Rust-idiomatic decision in the
  system.
- **Backpressure is free.** Bounded channels mean that if the GPU falls
  behind, workers block in `send` — automatically throttling self-play to
  GPU speed, no watchdog code.
- **Replies are oneshot channels**, one per request, carried inside the
  request itself. The service needs no request-table bookkeeping.

### Phased vs. continuous: the GPU contention answer

AlphaZero trains continuously while self-play runs — on separate
hardware. ELF ran continuous on shared hardware and reported >5×
self-play throughput for asynchronous pipelines, at the cost of
heterogeneous games (moves by different network versions in one game) and
stale BatchNorm moments (fixed by recomputing BN statistics from 50
batches every 1,000 updates) [paper — ELF].

We choose **phased v1**, following the alpha-zero-general `Coach.py`
loop structure:

```text
phase A: self-play N games   (GPU: evaluator only)
phase B: train K steps       (GPU: trainer only)
phase C: arena evaluation    (GPU: evaluator only, both agents)
repeat
```

Reasoning: on one GPU, "simultaneous" self-play and training means two
clients contending on one wgpu queue; the driver serializes them anyway,
just less predictably, and batch shapes start fighting (training wants
1024, evaluation wants ≤128). Phases cost us nothing in correctness,
make every number reproducible per iteration, and keep the first working
version debuggable. The upgrade path to continuous mode is designed-in
(evaluator and trainer are already separate actors; `gomoku run
--continuous` is a scheduling change, not a rewrite) and comes with ELF's
caveats taped to the lid: minimum replay size before training starts,
staleness budget for games, BN moment recompute. We go continuous when
profiling shows phase boundaries costing >10% throughput — not before.

### The replay buffer

Two candidate designs, decided:

- *Store positions* (planes + π + z): ~1 KB each, simple, but planes are
  derivable data.
- **Store games** (move list + outcome + metadata): ~60 bytes per game
  as `Vec<u8>` + header. The sampler draws a random game, replays it to a
  random ply through the engine (microseconds), encodes planes on the
  fly, applies a random symmetry, emits the sample.

Chosen: **store games**. 2 million games ≈ 120 MB — one-thousandth of
our RAM — versus ~2 TB for the equivalent positions. Deriving planes at
sample time costs CPU we have in abundance (the trainer thread is
GPU-bound, not CPU-bound) and buys flexibility: change the plane encoding
(chapter 10's history-plane experiments) and every stored game remains
valid. This is the batcher discipline from tutorial chapter 5 applied at
system scale: store raw, encode at the boundary.

Concurrency: appends come from 14 workers, sampling from 1 trainer. A
sharded ring buffer — one `Mutex<VecDeque<Game>>` per worker shard,
sampler round-robins shards — keeps lock hold times at nanoseconds. No
lock-free structure; contention is measured in microseconds per game, not
per operation. Persistence: each finished game is also appended to a
`bincode`-encoded journal file per iteration, so a crash costs at most
the current phase.

### What throughput to expect (order-of-magnitude, [derived])

The v1 network (next chapter) costs ~1.7 GFLOPs per evaluation. Assume
conservatively 5 effective TFLOP/s for batched inference through
wgpu/Metal (Apple publishes no number; measure in week one): ~2,950
evals/s. At 400 simulations per move [experiment], that is ~7 moves/s
across all workers; a ~50-move game every ~7 s wall-clock; **~450–600
games/hour**; a 25k-game iteration roughly every two days. Compare:
AlphaGomoku claims human level in two days on one (2018-era) GPU with a
far smaller net [paper]. Our net is bigger, our hardware newer — plan for *days to the first non-embarrassing agent, weeks
to a strong one*, and distrust any plan more precise than that.

---

## 10. The neural network

### Input planes

Per position, the engine's encoder emits `u8` plane arrays of shape
`[C, 17, 17]` (Burn-free; `net` converts them to `Tensor<B, 4>` of
shape `[batch, C, 17, 17]`). The planes are 17×17, not 15×15: the
border ring is always set in the "you" plane, so same-padding
convolutions read the board edge as a wall of opponent stones — exactly
how a wall constrains lines (chapter 7, the learning-side edge
problem). The border is never a legal move target, so the policy head
still emits 225 logits. Plane count (2 vs. the 4 below) is finalized
when the `net` crate lands — the store-games decision (chapter 9) keeps
both choices retro-compatible. Starting planes: 4.

| Plane | Content |
|---|---|
| 0 | stones of the player to move ("me") |
| 1 | stones of the opponent ("you") |
| 2 | my last move (single cell) |
| 3 | your last move (single cell) |

Reasoning: AlphaGo Zero uses 17 planes (8-step history × 2 + color)
[paper]; AlphaGomoku uses 3 [paper]. Gomoku's tactics are extremely
recency-driven — threats live in the last few moves — but a full 8-step
history is Go-scale excess for a game lasting 30–70 moves total. The
two last-move planes are the cheap 80% of history: they let the network
attend to the just-played forcing move without learning to diff stone
planes. The color plane is unnecessary (relative encoding makes
side-to-move implicit; there is no komi). The full
history variant (2 + 2k planes) is a registered `[experiment]` — cheap to
try later because we store games, not planes (chapter 9).

### Body: the residual tower

AlphaZero topology, scaled to Gomoku and one GPU:

```text
[B,4,17,17]
  └─ Conv2d 3×3, 4→128, same-pad ─ BN ─ ReLU
  └─ 10 × ResidualBlock:
        Conv2d 3×3 128→128 ─ BN ─ ReLU
        Conv2d 3×3 128→128 ─ BN ─ (+skip) ─ ReLU
  ├─ POLICY HEAD:  Conv2d 1×1 128→2 ─ BN ─ ReLU
  │                flatten [B,578] ─ Linear 578→225        (logits; the border
  │                                                         is never a legal target)
  └─ VALUE HEAD:   Conv2d 1×1 128→1 ─ BN ─ ReLU
                   flatten [B,289] ─ Linear 289→256 ─ ReLU
                                    Linear 256→1 ─ tanh    ([-1,1])
```

[derived] Sizes, computed for this document:

| Configuration | Parameters | FLOPs/eval |
|---|---|---|
| 64ch × 6 blocks | 0.66 M | 0.26 G |
| **128ch × 10 blocks (v1)** | **3.17 M** | **1.70 G** |
| 192ch × 12 blocks | 8.19 M | 4.61 G |
| 256ch × 20 blocks (ELF-class) | 23.83 M | 13.63 G |

Convolution FLOPs scale with board area (289/225 ≈ 1.28 versus a 15×15
input); parameters grow only in the two head linears (+45k, independent
of body size).

Why 128×10: AlphaGomoku reached human level with a *32-filter, 2-block*
net [paper] — Gomoku is tactically shallower than Go. KataGo's
progression — (6,96) → (10,128) → (15,192) → (20,256), growing the
network as training data accumulates [paper — KataGo] — tells us the
right mental model: network size is a *schedule*, not a decision.
(10,128) is KataGo's second rung and our v1: big enough to be clearly
stronger than AlphaGomoku's net, small enough that a forward-backward
pass on one sample costs ~5 GFLOPs [derived] and the replay window stays
meaningful at our game rate. Blocks and channels are `net` config
values; growing the net later = new config + warm-start from the old
record where shapes allow.

Every layer here is one we used in the MNIST tutorial (`Conv2d`,
`BatchNorm`, `Linear`, `AdaptiveAvgPool` unused — the heads flatten
directly, AlphaZero-style). The residual add is `x + block(x)` on
tensors. The module is a `#[derive(Module)]` struct with a
`#[derive(Config)]` — chapters 3 and 6 of the tutorial, verbatim
patterns, one type parameter away from CPU or GPU.

One deliberate omission: **no global pooling** (KataGo's value/policy
heads pool mean + scaled-mean + max per channel, 1.60× ablation factor
[paper — KataGo]). It is the first upgrade on the list after v1 trains —
it needs KataGo-style head surgery we should do with a working baseline
in hand, not speculatively.

### Loss and training step

`L = (z − v)² − πᵀ log p + c·‖θ‖²`  [paper — AGZ/AlphaZero form]

In Burn (chapter 4 manual loop): `MseLoss` on the value head,
cross-entropy of the *visit-count distribution* π against policy logits
(soft targets — π is a distribution, not an index, so this is the one
place we hand-roll: `-(pi * log_softmax(logits)).sum()`), and weight
decay through the optimizer. Optimizer: **AdamW, weight decay 1e-4**
(1e-4 is the AGZ L2 constant [paper]; with AdamW it is decoupled weight
decay — same spirit, better behaved at our batch size). The papers used
SGD+momentum 0.9 [paper — AGZ]; at DeepMind batch sizes (2048–4096) SGD
is canonical, but at our scale AdamW converges faster and is the same
optimizer family we validated in the tutorial. SGD+momentum stays as a
config switch for fidelity experiments. `[experiment]` either way — no
paper settles this for 1-GPU Gomoku.

Learning rate: linear warmup over ~2k steps, cosine decay 3e-4 → 3e-6
per iteration — the pattern from the official Burn MNIST example
(chapter 7), because paper schedules (0.2 → 0.0002 [paper — AlphaZero])
are tuned for SGD at batch 4096 and do not transfer.

### Records and checkpoints

Full-precision recorders only (`NamedMpkFileRecorder` /
`DefaultRecorder`): the tutorial's chapter-11 trap — `CompactRecorder`'s
f16 rounding — is disqualifying here, because arena comparisons between
candidate and baseline must reflect training, not quantization noise.
Checkpoint every iteration; keep all (disk is cheap; a corrupted run
restarts from `iter-17`).

---

## 11. Training configuration and the experiment budget

The table is the contract. Left column paper-anchored, right column ours
to tune. Tuning budget: **one variable per iteration**, always against
the arena Elo, always logged in the run journal.

| Parameter | v1 value | Source / reasoning |
|---|---|---|
| MCTS simulations / move | 400 | papers: 800 (AlphaZero), 1600 (AGZ) [paper]; 400 halves self-play cost while the net is still weak [experiment] |
| `c_puct` | 1.5 | **not published by DeepMind** — 1.5 is ELF OpenGo's value [paper — ELF]; tune ±0.5 [experiment] |
| Dirichlet ε | 0.25 | root-noise mixture weight, AGZ/KataGo form `0.75·p + 0.25·η` [paper — KataGo describes matching AlphaZero] |
| Dirichlet α | 0.1 | [derived]: AlphaZero's α values ≈ 10/(typical legal moves): Go 0.03@361, shogi 0.15@~80, chess 0.3@~35 [paper]. Gomoku midgame ≈ 100–150 legal moves → 10/125 ≈ 0.08; round to 0.1 [experiment] |
| Temperature | τ=1 for first 12 moves, then ≈0 | AGZ: 30 moves on ~200-move Go games [paper]; scaled to ~60-move Gomoku games ≈ 9–12 [derived] [experiment] |
| Resignation | on, calibrated to 5% false-positive rate | method from AGZ [paper — via ELF]; threshold recalibrated each iteration from arena games |
| Minibatch | 1024 | papers: 2048 (AGZ/ELF), 4096 (AlphaZero) [paper]; 1024 fits our GPU + data rate [experiment] |
| Replay window | start 250k positions, grow sublinearly | AlphaZero does **not** publish a window size; AGZ used 500k games [paper — via ELF]; KataGo grows 250k → 22M samples with a sublinear formula [paper — KataGo]. We adopt KataGo's growth rule |
| Minimum buffer before training | 20k positions | ELF's lesson: training on a tiny early buffer overfits instantly [paper — ELF] |
| Symmetry augmentation | 1 random D4 transform per sample | [paper — AlphaZero] |
| LR schedule | warmup 2k steps → cosine 3e-4→3e-6 | [experiment], pattern from Burn's official MNIST |
| Weight decay | 1e-4 (AdamW, decoupled) | constant from AGZ's L2 [paper], mechanism modernized |
| Gating | none (AlphaZero-style continuous replace) + arena Elo for observability | [paper — AlphaZero dropped gating]; AGZ's 55%-over-400-games gate [paper — via ELF] available as config if the run looks unstable |
| Playout-cap randomization | **off in v1** | KataGo: 25% of turns get full search, rest fast+unrecorded; worth ~1.37× [paper — KataGo]. First registered upgrade — it multiplies cheap games — but it changes the data distribution and we want a clean baseline first |

Registered upgrades, in order: (1) playout-cap randomization,
(2) global-pooling heads, (3) continuous mode, (4) network growth to
(15,192), (5) KataGo's forced-playouts-and-pruning (1.25× [paper]),
(6) auxiliary targets (opponent-next-move policy is the one that
transfers to Gomoku; score/ownership targets are Go-specific).

---

## 12. Verification and risk

Nothing trains until these pass. Order matters.

1. **Engine property tests** (proptest): fast bitboard ops vs. naive
   array reference — win detection, legality, symmetry transforms
   (applying any D4 transform twice in inverse order = identity; policy
   vector and planes transform identically).
2. **MCTS tactical tests**: hand-built threat puzzles (win-in-1,
   win-in-3 forced sequences, must-block open fours) with the engine's
   tactics module as mock evaluator. MCTS with a sane prior must solve
   win-in-1 at 50 simulations.
3. **TSS soundness gate**: every line the prover emits is re-verified by
   replaying it while enumerating all defender alternatives (cheap —
   replies are forced); on shallow random positions, brute-force
   adjudication must agree with every proof claim. The prover may be
   incomplete (miss wins); it may never be unsound.
4. **Network shape test + overfit test**: forward shapes on both
   backends; overfit 32 positions to >95% policy accuracy in a few
   hundred steps (proves loss, encoding, and training loop end-to-end).
5. **Backend parity gate** — the Metal risk made executable: same
   weights, same batch, Flex vs. Wgpu: forward outputs within 1e-5
   (f32), **and one full training step**: parameter deltas within 1e-4
   relative. Includes 1×1 convs (our heads) with autotune both on and
   off (issue #5626 pattern). If this fails: train on Flex, evaluate on
   Wgpu, file the bug, revisit next Burn release.
6. **Determinism policy**: `--seed` fixes worker RNG streams and Burn's
   backend seed; wgpu kernels are not bitwise deterministic across
   autotune choices, so determinism is promised on Flex, best-effort on
   Wgpu — recorded in the run journal either way.

Risk table (top 3): Metal training correctness → gate #5 + Flex
fallback. Learning stall (AlphaGomoku needed a curriculum) → the
tactics module mitigates it from day one and generates the synthetic
attack/defense set that milestone 3 trains on before self-play exists;
that milestone's acceptance test detects a stall early [paper], and the
milestone-4 anchor set keeps proven tactics in every batch thereafter.
Throughput over-optimism → the
chapter-9 numbers are [derived]; week-one measurement recalibrates
worker count, batch window, and sims/move before any long run.

---

## 13. The runbook: milestones to a running system

Each milestone ends with something executable and a pass/fail criterion.
Estimated effort assumes focused evenings, not DeepMind clusters.

| # | Milestone | Acceptance test |
|---|---|---|
| 1 | `engine` + tests | property tests green; perft-style move counts match reference for 10k random games; TSS soundness gate (§12 item 3) at 100%; `cargo bench` win-detection ≥ 50M checks/s |
| 2 | `mcts` + mock evaluator | solves tactical suite; plays legal full games vs. uniform-random evaluator without crashing (1000 games) |
| 3 | `net` + `train` on synthetic data (+ phase-0 external supplements per [ch. 14](14-openings-and-supervised-curriculum.md) decisions D2/D3) | overfit test passes; learns the tactics-generated synthetic attack/defense set (>90% top-1 on held-out synthetic threats) — proves the whole Burn path before self-play exists |
| 4 | TSS oracle + anchor set | soundness gate (§12 item 3) green; ≥10k machine-verified forced-win puzzles from random-play and Swap2 roots; a net trained with a small anchor fraction solves >95% of held-out proven puzzles without regressing the milestone-3 synthetic benchmark |
| 5 | `selfplay` + evaluator service (Swap2 opening procedure per [ch. 14](14-openings-and-supervised-curriculum.md) decision D1) | 14 workers, queue depth stable, ≥400 games/hour measured (recalibrate chapter 9) |
| 6 | **The loop, v1**: `gomoku run` phased | 10 iterations complete unattended; arena Elo of iteration *k* vs. iteration 0 is strictly, monotonically-ish increasing; first agent that beats raw-MCTS-only (>90% over 200 games) |
| 7 | Hardening | 7-day continuous run without intervention; crash-recovery from journal verified by kill -9 drill |
| 8 | Registered upgrades, one at a time | each lands only if arena Elo improves beyond noise (±30) |

**Why milestone 4 is a step of its own.** TSS labels are exact, not
bootstrapped: policy = the forcing move, value = rules-truth ±1. That is
supervised signal of a quality self-play will not produce for a long
time, aimed at the network's weakest spot — sharp tactical lines where a
weak prior makes MCTS blunder. The anchor set is *permanent*: it lives
outside the replay window (no eviction), contributes a small
[experiment] fraction of every batch from milestone 4 onward, and is
regenerated as self-play improves — puzzle roots come from random play
and Swap2 openings at first, from real self-play games later, so the set
tracks the distribution the agent actually visits. Two guards keep it
honest: **soundness over completeness** (only machine-verified lines
become labels — §12 item 3), and a *small* fraction, because forcing
positions are a narrow, weird slice of the game and overfeeding them
warps the policy in quiet positions. External puzzle collections can
validate the prover but are not bulk training data (appendix). Later, a
registered upgrade can call the same prover *inside* MCTS — proven
nodes back up exact ±1 instead of the network's value, the classic
strong-Gomoku shortcut; that is milestone-8 material, deliberately not
v1.

After milestone 6 we are where DeepGomoku once was — but in Rust, tabula
rasa, with a design that grows. Everything after that is compute and
patience.

---

## 14. Appendix: sources and the honesty ledger

Papers (links verified 2026-09-14):

- AlphaGo (2016): storage.googleapis.com/deepmind-media/alphago/AlphaGoNaturePaper.pdf — APV-MCTS, virtual loss usage
- AlphaGo Zero (2017): doi.org/10.1038/nature24270 — the algorithm; no arXiv version exists
- AlphaZero (2018): arxiv.org/abs/1712.01815 — 800 sims, 700k steps × 4096, lr 0.2→0.0002, Dirichlet α per game, no gating, TPU counts
- ELF OpenGo (2019): arxiv.org/abs/**1902.04522** — 20×256 net, batch 2048, 500k-game window, 1600 rollouts, c_puct=1.5, async lessons, BN-moment fix, batch cap 128. (Watch the ID: 1902.10565 is KataGo.)
- KataGo (2019): arxiv.org/abs/**1902.10565** — 50× efficiency, playout-cap randomization, forced playouts + pruning, global pooling, growing nets and windows, ablation factors
- MuZero (2020): arxiv.org/abs/1911.08265 — beyond scope, north star
- AlphaGomoku (2018): arxiv.org/abs/1809.10595 — 3×15×15 input, 32-filter net, curriculum, 2-day claim
- MCTS review: arxiv.org/abs/2103.04931 — parallelization taxonomy and attributions
- Allis thesis (1994): DOI 10.26481/dis.19940923la (Maastricht); Allis, van den Herik & Huntjens (1993), *Go-Moku and Threat-Space Search*, University of Limburg — freestyle Gomoku is a first-player win
- Renju rules: renju.net/rifrules; Swap2 status: Wikipedia (unsolved under Swap2)
- TSS puzzle data — leads only, **unverified** (no web access during
  writing; verify before relying on any of these): Gomocup tournament
  game archives (gomocup.org) as a source of strong-engine root
  positions; the renju.net game database (Renju rules — forbidden moves
  and no overline win for Black — so positions need re-adjudication
  under freestyle rules); community VCF exercise collections in
  piskvork-era formats, provenance and licensing unclear.
- Apple M5 Max: apple.com/newsroom/2026/08 (Mac Studio announcement) + apple.com/mac-studio/specs
- Burn issues: #5162 (Metal NaN gradients), #5626 (autotune 1×1 conv), burn v0.21.0 tag + Cargo.lock (wgpu 29 via CubeCL 0.10)

**The honesty ledger** — values the folklore gets wrong, stated plainly:

1. `c_puct` is **not** in the AlphaGo Zero or AlphaZero papers. 1.5 is
   ELF OpenGo's choice. Ours is an experiment parameter anchored at ELF's
   value.
2. AlphaZero's replay-window size is **not published**. The 500k-games
   figure is AlphaGo Zero's (via ELF's comparison table); KataGo's
   growing-window rule is what we adopt.
3. "AlphaGo Zero used 800 simulations" — no: 800 is AlphaZero (all
   games); AGZ used 1,600 (via ELF's table).
4. Apple publishes no Neural-Engine TOPS or GPU fp16 TFLOPS for M5 Max.
   Every throughput number in chapter 9 is [derived] from our network's
   FLOPs and a conservative efficiency guess — to be replaced by
   measurement in week one.
5. The Allis thesis *metadata* is verified; the thesis PDF itself was
   retrieved on 2026-09-20 (see [the reference
   catalogue](references/catalogue.md)), resolving this entry.
6. The TSS dataset list above was a set of *leads* when written (no
   web access then); it is RESOLVED as of 2026-09-20 — a verified
   sweep catalogued the usable sources
   ([findings](references/findings-external-data.md)) and mirrored
   them locally under `data/`. Our primary puzzle source remains
   self-generated (random play, Swap2 openings, later self-play):
   renewable, license-free, and on-distribution.

Next: milestone 1, the engine. That is where the code begins.

Back to: [Wiki home](README.md) · Previous: [Chapter 11 — Pitfalls](11-pitfalls.md)
