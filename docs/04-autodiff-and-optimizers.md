# Chapter 4 — Autodiff and optimizers

Run the example first: `cargo run --example 03_autodiff`

## Autodiff is a backend decorator

```rust
type B  = burn::backend::Flex;      // computes
type AB = burn::backend::Autodiff<B>; // computes AND records a gradient tape
```

`Autodiff<B>` wraps any backend. Its tensors behave like normal tensors, but
every float operation is also recorded on a tape. The trait bound
`B: AutodiffBackend` marks code that needs `.backward()`.

Consequences:

- **Inference code cannot build a graph by accident.** It is generic over
  `Backend`, and plain backends have no `backward()`. This is Burn's
  type-level replacement for `torch.no_grad()` and `model.eval()`.
- `model.valid()` converts `Model<Autodiff<B>>` into `Model<B>` (inner
  backend, eval mode) for validation inside a training loop.
- `Autodiff` composes with everything: `Autodiff<Wgpu>` trains on GPU with
  no other code change.

## The gradient flow, by hand

There is no `.grad` field mutated in place as in PyTorch. The flow is
explicit:

```rust
let loss = mse(model.forward(x), y);   // 1. forward, scalar loss
let grads = loss.backward();           // 2. reverse-mode autodiff -> B::Gradients
let grads = GradientsParams::from_grads(grads, &model); // 3. map to parameter ids
model = optim.step(lr, model, grads);  // 4. optimizer consumes module, returns it
```

Step 2 returns an opaque gradient map keyed by tensor id. Step 3 ties those
gradients to the module's `Param` fields. Step 4 is functional style: the
optimizer **consumes** the module and returns an updated one. Burn modules
are value types; "updating weights" means replacing the value.

For leaf tensors outside a module (rare), use `.require_grad()` to opt a
tensor into gradient tracking, then read its gradient with
`x.grad(&grads)`. Example 03 shows both: one scalar derivative computed this
way, then a full manual training loop through a module.

## What the manual loop teaches you

Example 03 fits `y = 2x + 1` in 200 manual steps. This exact loop — forward,
backward, map gradients, step — is what `SupervisedTraining` automates in
chapter 6. Learn it well. In the AlphaZero project you will write a custom
loop again, because self-play data generation does not fit the
dataset-driven training paradigm. When that day comes, this chapter is the
template.

## Optimizers and learning rate schedules

Optimizers follow the config pattern: `AdamConfig::new().init()`,
`SgdConfig::new().init()`, `AdamWConfig::new().with_weight_decay(5e-5).init()`.

`Learner::new(model, optim, lr)` takes the learning rate as an
`LrScheduler`. A plain `f64` is a valid constant schedule. Composable
schedulers live in `burn::lr_scheduler`: `LinearLrSchedulerConfig`,
`CosineAnnealingLrSchedulerConfig`, `ComposedLrSchedulerConfig` (warmup +
decay, see the advanced MNIST example in chapter 7).

## Reading

- Book:
  [Autodiff](https://burn.dev/books/burn/building-blocks/autodiff.html),
  [Custom training loop](https://burn.dev/books/burn/custom-training-loop.html)

## Try this

1. In example 03, change the target function to `y = 3x - 2` and confirm the
   model learns it.
2. Replace SGD with Adam (`AdamConfig`) and count how many fewer epochs the
   fit needs.
3. Compute `d/dx sin(x) * x^2` at `x = 1` with `require_grad` and check the
   result against a hand calculation.

Next: [Chapter 5 — The data pipeline](05-data-pipeline.md)
