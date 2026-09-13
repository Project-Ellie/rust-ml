//! Chapter 1 — Tensors and backends.
//!
//! Run: `cargo run --example 01_tensors`
//!
//! Companion doc: `docs/02-tensors-and-backends.md`.
//!
//! This example stays on the Flex backend (pure-Rust CPU, Burn's recommended
//! CPU path for new projects). Everything you see here works identically on
//! any other Burn backend — only the type alias and the device would change.

use burn::backend::Flex;
use burn::tensor::{Int, Tensor, TensorData};

// The backend is a type, not a runtime object. `Flex<E, I>` is generic over
// the float element type (default f32) and the int element type (default i32).
type B = Flex;

fn main() {
    // Every backend has an associated `Device` type. Flex has a single CPU
    // device; GPU backends would enumerate hardware here.
    let device = Default::default();

    // --- Creation ------------------------------------------------------------

    // `Tensor<B, D>` — the rank D is a const generic, checked at compile time.
    // A 2x3 matrix of floats:
    let a = Tensor::<B, 2>::from_floats([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]], &device);

    // The more general constructor goes through `TensorData`, the
    // backend-independent host-side container (this is how datasets enter the
    // tensor world):
    let data = TensorData::from([[1_i32, 2, 3], [4, 5, 6]]);
    let b = Tensor::<B, 2>::from_data(data.convert::<f32>(), &device);

    // Integer tensors use the `Int` kind marker as third generic parameter:
    let labels = Tensor::<B, 1, Int>::from_data([3_i64, 1, 4], &device);

    // --- Shape and dtype introspection ----------------------------------------

    println!("a.shape()        = {:?}", a.shape());
    println!("a.dims()         = {:?}", a.dims()); // [2, 3] as an array
    println!("labels.dtype()   = {:?}", labels.dtype());

    // --- Operations ------------------------------------------------------------

    // Element-wise ops consume and return tensors; `clone` where you need both.
    let c = a.clone() + b.clone(); // [2, 3]
    let d = a.clone().mul_scalar(2.0); // scalar ops come as *_scalar variants
    println!("a + b            = {c}");
    println!("a * 2            = {d}");

    // Matrix multiplication is `matmul` — shapes must line up, and shape
    // errors are *runtime* errors (only the rank is compile-time checked).
    let e = Tensor::<B, 2>::ones([3, 4], &device);
    let f = a.clone().matmul(e); // [2, 3] @ [3, 4] -> [2, 4]
    println!("a @ ones([3,4])  = {f}");

    // --- Reshaping and combining ------------------------------------------------

    // Rank changes are visible in the type: `reshape` infers the new rank from
    // the target shape array length.
    let g: Tensor<B, 3> = a.clone().reshape([1, 2, 3]);
    println!("reshaped         = {:?}", g.shape());

    // Concatenation along an existing dimension:
    let h = Tensor::cat(vec![a.clone(), b.clone()], 0); // [4, 3]
    println!("cat dim 0        = {:?}", h.shape());

    // Slicing uses Rust ranges per dimension:
    let row0 = a.clone().slice([0..1, 0..3]);
    println!("a[0, :]          = {row0}");

    // --- Reductions ---------------------------------------------------------------

    println!("a.sum()          = {}", a.clone().sum());
    println!("a.mean_dim(0)    = {}", a.clone().mean_dim(0));

    // --- Getting values back out ---------------------------------------------------

    // `into_data` blocks until the value is on the host (matters on async GPU
    // backends; free on CPU backends). `into_scalar` extracts a single element.
    let total: f32 = a.sum().into_scalar();
    println!("total as f32     = {total}");

    // --- The one-sentence summary ---------------------------------------------------
    //
    // Tensors are typed `Tensor<Backend, Rank, Kind>`; all neural network code
    // in Burn is generic over `Backend`, so this entire example runs unchanged
    // on Wgpu, Cuda, NdArray or any other backend by swapping the type alias.
}
