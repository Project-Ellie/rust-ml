# How to train your first model and play against it in the CLI

A step-by-step manual for the full loop: dataset → training run → a
trained model as an AI seat at the study table. Every command below
was run and verified on `main` (2026-11-08). Work from the `gomoku/`
directory unless a command says otherwise.

**Time budget:** the quick loop (small numbers below) takes about two
minutes. The "real" sizes take minutes for datagen and under an hour
for training, CPU only.

---

## Step 1 — create a dataset with `datagen`

Quick loop (500 samples, seconds):

```bash
cd gomoku
cargo run --release -p train --bin datagen -- \
    --out ../data/synthetic/v0 --seed 1 \
    --win 200 --block 200 --quiet 100
```

Real dataset (50k samples, the tutorial's acceptance size):

```bash
cargo run --release -p train --bin datagen -- \
    --out ../data/synthetic/v1 --seed 1 \
    --win 20000 --block 20000 --quiet 10000
```

What you get: a directory of bincode shards plus `manifest.json`.
Check the manifest — it is the corpus's identity card:

```bash
cat ../data/synthetic/v0/manifest.json
```

You should see your quotas echoed in `counts`, the shard list, and
the console report's holdout split (~10% of positions, by Zobrist
hash). The corpus is byte-identical if you regenerate it with the
same seed — never hand-edit it; delete and regenerate instead.

Exit codes (same contract everywhere in this project): `0` success,
`1` the tool ran but failed (e.g. unwritable directory), `2` you
invoked it wrong (usage error).

## Step 2 — train a model with `nettrain`

Quick loop (tiny net, 200 steps, about a minute):

```bash
cargo run --release -p net --bin nettrain -- \
    --out ../artifacts/gomoku/runs/first-model \
    --data ../data/synthetic/v0 \
    --config tiny --seed 1 --steps 200 --batch-size 32 \
    --eval-interval 50
```

Real run (mid-size net on the 50k corpus):

```bash
cargo run --release -p net --bin nettrain -- \
    --out ../artifacts/gomoku/runs/phase0-mid-v1 \
    --data ../data/synthetic/v1 \
    --config mid --seed 1 --steps 20000 --batch-size 64 \
    --eval-interval 500
```

Flags: `--out` (run directory, required), `--data` (corpus
directory; omit it for a 200-sample in-memory smoke run), `--config
tiny|mid|default` (8ch/1 block, 32ch/4 blocks, 128ch/10 blocks — the
locked architecture), `--device flex|wgpu` (see below), `--seed`,
`--steps`, `--batch-size`, `--eval-interval` (0 disables holdout
evals).

**Device:** default is `flex` — pure-Rust CPU, deterministic, the
only parity-verified training path. `--device wgpu` (Metal on macOS)
requires the `gpu` cargo feature:

```bash
cargo run --release -p net --features gpu --bin nettrain -- \
    --out ../artifacts/gomoku/runs/gpu-test \
    --data ../data/synthetic/v1 \
    --config mid --device wgpu --steps 20000 --batch-size 64
```

GPU training is an *experiment*, not a candidate factory: it is not
parity-verified against Flex (Burn wgpu issues #5162 NaN gradients,
#5626 autotune 1x1-conv both touch this network's layers) and loses
bit-exact reproducibility. Verified working as a smoke run
(2026-11-08, M5): 20 steps, finite decreasing loss, checkpoint
written.

**No resume — and re-running overwrites.** `nettrain` always starts
from a freshly initialized model, and `save_checkpoint` silently
replaces `model.mpk`/`run.json` in the `--out` directory. Re-running
the same command does not continue a run; it destroys it. One run
directory per run. (True resume needs optimizer state, which the
checkpoint does not store yet — milestone-7 crash-recovery territory.)

What you watch: progress goes to `stderr` (one line per step:
loss + learning rate; one holdout block per eval interval). The
final report goes to `stdout`:

```text
Train loss: initial=6.348772 final=6.205074 over 200 steps
Holdout @ step 200: loss=6.291445 policy_top1=0.00% value_sign=57.78%
```

What you keep: the run directory holds `model.mpk` (full-precision
model record) and `run.json` (the journal: seed, both configs, final
metrics). The journal is what makes two runs comparable — diff the
journals, not your memory.

Expectations for the quick loop: the loss barely moves — 455 train
samples and an 8-channel net are a *pipeline* proof, not a trained
player. The chapter-8 acceptance bar (≥80% policy-argmax agreement on
held-out win/block, ≥85% value-sign) is meant for the full synthetic
set at real sizes. If holdout loss climbs while train loss falls, the
net is memorizing — more data or fewer steps.

## Step 3 — play against the model in the CLI

```bash
cargo run --release -p cli
```

At the study table (typed command line):

```text
model ../artifacts/gomoku/runs/first-model
release white
sims white 100
```

Then play your move (e.g. `h8`) and `step` — or simply play; the AI
answers when its seat is up. What just happened:

- `model DIR` loads `run.json` (for the network configuration) and
  `model.mpk` (the weights) from the run directory and installs the
  network as the evaluator of **every** AI seat. The reply line
  confirms: `model loaded from … — AI seats now use the network`.
- `release white` gives the White seat to the AI (in a hotseat both
  seats start human). With `--ai` from the shell you can skip this.
- `sims white 100` keeps the search snappy: every simulation is a CPU
  forward pass through the network, so 400 simulations against the
  `default` config is seconds per move.

The overlays now show a trained mind: `v` (value strip), `p` (policy
visits/prior), `t` (TSS overlay) all work against the net seat.

What to expect from a synthetic-only model: tactical reflexes —
it takes wins-in-one and (sometimes) blocks yours — and no strategy.
It will feel like a sharp beginner. That is the correct first
artifact; the mentor diet (next tutorial) is what makes it
interesting.

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `nettrain failed: …` exit 1 | dataset dir missing/empty | check the `--data` path holds shards + `manifest.json` |
| usage error, exit 2 | bad flag | `nettrain` prints usage; flags are `--flag value`, never `--flag=value` |
| `could not load model from …` at the CLI | path is not a run directory | point `model` at the `--out` directory containing `model.mpk` **and** `run.json` |
| `device 'wgpu' requires the gpu feature` | GPU build missing | add `--features gpu` to the cargo invocation |
| `net support is not compiled in` | lean build | rebuild without `--no-default-features` (the `net` feature is default-on) |
| search is slow | CPU inference per simulation | `sims both 100`, or use a `tiny`/`mid` model |
| a failed `model` command changed my game | it didn't | loads are transactional: on error the session is untouched |

## v1 limitations (deliberate — the integration tutorial's job later)

- **Last-move planes are zero-filled.** The `mcts::EvalRequest` seam
  carries only the two engine planes (me/you); the network's history
  planes (net tutorial chapter 1) see zeros. The model plays slightly
  below its trained strength.
- **The net seat is runtime-only.** Save files keep the seats'
  configured evaluator kinds (`tactics`/`baseline`); a `model`
  installation does not travel with `save`/`load`. Re-issue `model`
  after loading a study.
- **No per-seat networks and no unload.** Every AI seat shares the
  installed model; restart the CLI to go back to the configured
  evaluators.
- **The model file is burnpack format** under the historical name
  `model.mpk` (Burn 0.22 replaced the recorder API). Nothing parses
  the extension, so this is cosmetic.

## Where this sits in the journey

This manual is the fast lane to "a real model on the table". The full
journey — mentor self-play corpora, the signal ladder, acceptance
bars, the arena — is sketched in
[specs/2026-11-08-phase-0-training-journey.md](specs/2026-11-08-phase-0-training-journey.md).car
