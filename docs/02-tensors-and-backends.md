# Chapter 2 — Tensors and backends

Run the example first: `cargo run --example 01_tensors`

## The type signature

```rust
Tensor<B, D>            // float tensor, backend B, rank D
Tensor<B, D, Int>       // integer tensor
Tensor<B, D, Bool>      // boolean tensor
```

- `B` is the backend. It is a type, not a value. You never construct it.
- `D` is the **rank** (number of dimensions), a const generic. The compiler
  rejects rank mismatches. A rank-2 tensor can have any 2D shape; shapes are
  checked at runtime.
- The **element type** (`f32`? `i64`?) is decided by the backend, not by the
  tensor type. `Flex` defaults to `f32`/`i32`; `NdArray` to `f32`/`i64`.
  Convert with `.elem::<B::FloatElem>()` / `.elem::<B::IntElem>()` when you
  construct tensors from Rust values in generic code.

This is different from TensorFlow/PyTorch, where dtype is part of the tensor.
In Burn, generic code must stay dtype-agnostic. Concrete code (like our
examples) can simply annotate the extracted type: `let x: f32 = t.into_scalar();`.

## TensorData: the host-side container

`TensorData` holds bytes, shape, and dtype on the host. It is the bridge
between ordinary Rust data (arrays, vectors, dataset items) and tensors:

```rust
let data = TensorData::from([[1.0, 2.0], [3.0, 4.0]]);
let t = Tensor::<B, 2>::from_data(data, &device);
```

Dataset items enter the tensor world through `TensorData` in the batcher
(chapter 5). `.convert::<E>()` changes the element type before upload.

## Ownership and cloning

Tensor operations consume `self`. If you need a tensor twice, clone it:

```rust
let c = a.clone() + b.clone();
```

A clone is cheap: it increments a reference count on the buffer, it does not
copy data. Burn's book is explicit about this. The practical rule: chain
operations on temporaries, clone at reuse points, and do not fear clones in
model code.

## Devices

Each backend has an associated `B::Device` type. Flex has a single CPU
device, so `Default::default()` is all you need. GPU backends enumerate
hardware (`WgpuDevice::default()`, `CudaDevice::new(0)`, ...). Tensors are
created on a device; operations require matching devices.

## Operations you will use daily

| Operation | Example |
|-----------|---------|
| element-wise | `a + b`, `a.mul_scalar(2.0)` |
| matrix product | `a.matmul(b)` |
| reshape (rank change is typed) | `t.reshape([b, 1, 28, 28])` |
| concatenate | `Tensor::cat(vec![x, y], 0)` |
| slice | `t.slice([0..2, 1..3])` |
| reductions | `t.sum()`, `t.mean_dim(0)` |
| softmax | `burn::tensor::activation::softmax(t, 1)` |
| comparison | `a.equal(b)` → `Tensor<B, D, Bool>` |
| cast | `bool_t.int()`, `.float()` |
| extraction | `t.into_scalar()`, `t.into_data().to_vec::<f32>()` |

Two traps this course hit, so you do not have to:

1. **`argmax` keeps the reduced dimension.** `output.argmax(1)` on
   `[batch, 10]` gives `[batch, 1]`, not `[batch]`. Squeeze it:
   `.argmax(1).squeeze_dim::<1>(1)`.
2. **`into_scalar()` returns the backend element type.** On Flex that is
   `i32` for int tensors, not `i64`. Annotate accordingly.

## Backend selection

Application code is generic: `fn forward<B: Backend>(&self, x: Tensor<B, 2>)`.
The binary picks a concrete backend with a type alias:

```rust
type B = burn::backend::Flex;            // pure-Rust CPU (this course)
// type B = burn::backend::Wgpu;         // portable GPU (feature "wgpu")
// type B = burn::backend::NdArray;      // legacy CPU (feature "ndarray")
```

For training you use the autodiff-decorated twin: `type AB = Autodiff<B>;`
(chapter 4). Burn 0.21 also has a `Dispatch` mechanism for runtime backend
selection; the official MNIST example uses it. This course stays with
compile-time selection — simpler, and the compiler monomorphizes your model
for exactly one backend.

## Reading

- Book: [Tensor](https://burn.dev/books/burn/building-blocks/tensor.html),
  [Backend](https://burn.dev/books/burn/building-blocks/backend.html)

## Try this

1. In `01_tensors.rs`, multiply two tensors with mismatched ranks and read
   the compiler error. Then mismatch the *shapes* with equal ranks and read
   the runtime error. Learn to tell the two apart.
2. Write a function `fn batch_means<B: Backend>(t: Tensor<B, 3>) -> Tensor<B, 2>`
   that averages over the last dimension. Call it from the example.
3. Change the type alias in `01_tensors.rs` to `NdArray` (add the `ndarray`
   feature) and confirm that everything else compiles unchanged. Then change
   it back — Flex is the recommended CPU backend.

Next: [Chapter 3 — Modules](03-modules.md)
