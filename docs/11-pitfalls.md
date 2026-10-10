# Chapter 11 — Pitfalls

Known traps with Burn. The gomoku workspace is on **Burn 0.22.0**; the root
MNIST curriculum stays on **Burn 0.21.0**. The first four were hit while
building this repository. The rest come from Burn's issue tracker and
community reports (links included).

## Hit during the making of this course

1. **`CompactRecorder` stores f16.** A saved-and-reloaded model is not
   bit-identical: outputs differ by ~1e-4. For exact round-trips (resume
   training, deterministic tests) use `DefaultRecorder` or
   `NamedMpkFileRecorder<FullPrecisionSettings>`.
   [Source](https://github.com/tracel-ai/burn/blob/v0.22.0/crates/burn-core/src/record/recorder.rs)
2. **`argmax` keeps the reduced dimension.** `[batch, 10].argmax(1)` is
   `[batch, 1]`. Comparing against `[batch]` targets fails with a confusing
   type error. Squeeze: `.argmax(1).squeeze_dim::<1>(1)`.
3. **Int element type differs by backend (and by Burn version).** In Burn
   0.22, Flex and Wgpu both use `i32`. The deprecated NdArray backend used
   `i64`. In the gomoku workspace, annotate the expected type on
   `into_scalar()` (e.g., `into_scalar::<i32>()`). In the 0.21 MNIST code,
   generic code must use `.elem::<B::IntElem>()`.
4. **Stale API names in old tutorials.** `LearnerBuilder`, the `Infer`
   trait, and `InitRecord` are gone in 0.21. The current names are
   `SupervisedTraining` + `Learner`, `InferenceStep`, and
   `Config::init` + `load_record`. Any blog post using the old names is
   pre-0.21 — often still conceptually right, mechanically wrong.

## From the Burn issue tracker

5. **Do not follow `main`.** Burn's `main` branch is the next prerelease and
   may change APIs that the pinned releases rely on. The gomoku workspace
   pins `=0.22.0`; the root MNIST package pins `=0.21.0`. Code from `main`
   will not compile against either pin. [Issue
   #4879](https://github.com/tracel-ai/burn/issues/4879)
6. **Binary records are not forward-compatible.** Burn minor releases often
   break binary record compatibility (for example, 0.21 changed
   `TensorData`'s shape type; 0.22 may introduce further changes). Older
   binary records need conversion. Test loading your artifacts after every
   Burn upgrade. [Burn 0.22 release
   notes](https://github.com/tracel-ai/burn/releases/tag/v0.22.0)
7. **A bare `Tensor` field in a module is a constant.** Learnable raw
   weights must be `Param<Tensor<...>>`, or they are neither trained nor
   saved. Non-module fields need `#[module(skip)]`.
8. **Batcher dtype/shape discipline.** Targets for `CrossEntropyLoss` are
   `Tensor<1, Int>` class indices on the correct device. Most beginner
   data-pipeline bugs live in the batcher.
   [Issue #1825](https://github.com/tracel-ai/burn/issues/1825)
9. **Custom metrics must weight by batch size.** Epoch aggregation is
   sample-weighted; a custom metric that averages per-batch values is
   silently biased by the short last batch.
10. **GPU backends are not yet numerically boring.** Reports include wrong
    1x1-conv results under autotune on Metal
    ([#5626](https://github.com/tracel-ai/burn/issues/5626)) and a missing
    sync boundary in blockwise ops
    ([#5563](https://github.com/tracel-ai/burn/issues/5563)). Rule: develop
    on Flex, and add a small CPU/GPU parity test before trusting a new
    backend.
11. **WebGPU WASM gaps.** e.g. `topk_with_indices` panics on the WASM WebGPU
    backend ([#5110](https://github.com/tracel-ai/burn/issues/5110)). Check
    op support before designing for the browser.
12. **ONNX/PyTorch import is not a universal converter.** `burn-onnx`
    supports an operator subset; `burn-store` loads weights into an
    architecture you must still write in Burn. Check the [operator
    matrix](https://github.com/tracel-ai/burn-onnx/blob/main/SUPPORTED-ONNX-OPS.md)
    and validate outputs against the source runtime.

## The meta-pitfall

Burn moves fast. The durable skills from this curriculum are the four ideas
of chapter 1 (typed tensors, module derive, config/state split, autodiff as
backend) plus the manual training loop of chapter 4. Those survive version
churn. Specific builder method names do not — when in doubt, read the source
of the pinned tag, not a search result.

Back to: [Wiki home](README.md)
