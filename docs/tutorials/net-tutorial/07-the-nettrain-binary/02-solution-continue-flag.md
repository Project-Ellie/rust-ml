# Chapter 07 — Extension solution: the `--continue` flag

This is an opt-in reference, scoped 2026-11-12. It is not part of the
verified chapter; it extends the `nettrain` binary from Chapter 07 with
one feature. Everything lives in `crates/net/src/bin/nettrain.rs`. No
other file changes.

## Intention

A finished run leaves `model.mpk` and `run.json` in `--out`. You point
`nettrain` at that same directory again and pass `--continue`. Training
then starts from the saved weights instead of random init, and the new
checkpoint overwrites the old one at the end. Without `--continue`,
behavior is exactly what Chapter 07 shipped: fresh init, overwrite at
the end.

Explicitly out of scope (decided, not forgotten):

- Optimizer state. AdamW starts fresh; its moment estimates rebuild
  within a few hundred steps.
- LR schedule continuity. Warmup + cosine restart from `lr_max` for the
  new leg. A continued run is a fresh schedule on warm weights.
- Step numbering. The new leg counts `1..=steps` again.
- Metrics continuity. `metrics.jsonl` is truncated for the new leg; the
  previous leg's stream is gone once you continue. Same for `run.json`:
  the journal describes the latest leg only.
- Mid-run checkpoints. `--continue` resumes from *finished* runs only.

If any of these start to matter, that is a new feature, not a bug in
this one.

## Specification as tests

Write these first, in the existing `mod tests` at the bottom of
`nettrain.rs`, in this order:

1. `parse_args_continue_defaults_to_false` — no flag → `false`.
2. `parse_args_continue_flag` — `--continue` (no value) → `true`.
3. `initial_model_errors_when_continue_without_checkpoint` — flag set,
   empty directory → `Err` mentioning there is nothing to continue
   from.
4. `initial_model_errors_on_config_mismatch` — checkpoint saved with
   `ModelConfig::tiny()`, args select a different config → `Err`
   naming both configs.
5. `initial_model_loads_checkpoint_when_continue` — checkpoint present,
   configs match → returned model produces bit-identical outputs to the
   saved model on a fixed input (mirror the roundtrip test in
   `checkpoint.rs`).
6. `initial_model_fresh_without_continue` — checkpoint present but flag
   absent → fresh init, no error.

Tests 3–6 need a real checkpoint on disk. Reuse the `temp_dir()` helper
already in the test module, and `checkpoint::save_checkpoint` with a
`ModelConfig::tiny()` model to create one — the same pattern as the
`checkpoint_saves_both_model_and_journal` test in `checkpoint.rs`.

## Walkthrough, in execution order

### Step 1: parse the flag

Every existing flag consumes a value via `args.next()`. `--continue` is
the first boolean: its match arm sets a variable to `true` and consumes
nothing.

```rust
"--continue" => {
    continue_run = true;
}
```

Declare `let mut continue_run = false;` with the other defaults at the
top of `parse_args`, add `continue_run` to the returned `Args`, and add
`[--continue]` to both usage strings (the doc comment at the top of the
file and the `eprintln!` in `main`). Tests 1 and 2 go green here.

Why a bare flag and not `--continue <DIR>`: the directory is always
`--out`. One path, one concept, nothing to keep consistent.

### Step 2: the `Args` field

```rust
/// Continue from the checkpoint in `out_dir` instead of fresh init.
continue_run: bool,
```

Adding the field breaks the two test constructors that build `Args`
literally (`args_with_data`). Let the compiler point you at them; add
`continue_run: false`. That compile error is your checklist.

### Step 3: resolve the initial model in `run()`

Currently line 208 of `nettrain.rs` is unconditional fresh init:

```rust
let model: Model = args.config.init(&device);
```

Replace it with a call to a small helper. Factoring it out is what
makes tests 3–6 possible — `run()` itself loads datasets and starts
reporters, so it is untestable at this granularity:

```rust
let model: Model = initial_model(&args, &device)?;
```

The helper, placed near `load_or_generate_data`:

```rust
/// The model to start training from: the checkpoint in `out_dir` when
/// `--continue` is set, fresh init otherwise.
fn initial_model(args: &Args, device: &Device) -> Result<Model, String> {
    if !args.continue_run {
        return Ok(args.config.init(device));
    }
    let journal = load_journal(&args.out_dir).map_err(|e| {
        format!(
            "--continue given but no checkpoint in {}: {e}",
            args.out_dir.display()
        )
    })?;
    if journal.model_config != args.config {
        return Err(format!(
            "--continue checkpoint was trained with {:?} but --config selects {:?}; \
             refusing to load mismatched weights",
            journal.model_config, args.config
        ));
    }
    eprintln!("nettrain: continuing from {}", args.out_dir.display());
    load_model(&args.out_dir, &journal.model_config, device)
        .map_err(|e| format!("failed to load checkpoint: {e}"))
}
```

Add `load_journal` and `load_model` to the existing
`use net::checkpoint::{...}` import. Tests 3–6 go green here.

Three decisions folded into this step:

- **Error on a missing checkpoint instead of falling back to fresh
  init.** The expensive failure mode is believing you continued a 20k-
  step run while actually training from scratch. A loud exit costs you
  one re-run; a silent fallback costs you the run.
- **Validate the config instead of trusting the journal.** The journal
  carries `model_config`, so the helper could ignore `--config`
  entirely. But then `--config tiny --continue` against a `default`
  checkpoint would silently load `default` weights. `ModelConfig`
  derives `PartialEq` (the existing parse tests already `assert_eq!`
  it), so the check is one `if`.
- **Fresh optimizer and schedule are features of the scope, not
  oversights.** `train_with_reporter` creates both internally; not
  touching `train.rs` at all is what keeps this a one-file change.

### Step 4: nothing else

`train_with_reporter` and `save_checkpoint` are untouched. The end of
`run()` already overwrites `model.mpk` and `run.json` in `out_dir`,
which is precisely the "new leg replaces old leg" behavior the
intention asks for.

## Done when

- All six tests above pass, plus the existing suite.
- Gates green: `cargo test`, `cargo clippy --all-targets -- -D warnings`,
  `cargo fmt --all`.
- Manual smoke, on a tiny config so it costs a minute:
  1. `nettrain --out /tmp/cont-test --steps 50` → note the final loss.
  2. `nettrain --out /tmp/cont-test --steps 50 --continue` → stderr
     shows `continuing from /tmp/cont-test`, and the *initial* loss of
     leg two is far below the initial loss of leg one.
  3. `nettrain --out /tmp/empty --continue` → fails with "no
     checkpoint".
  4. `nettrain --out /tmp/cont-test --config mid --continue` → fails
     with the config-mismatch error.
