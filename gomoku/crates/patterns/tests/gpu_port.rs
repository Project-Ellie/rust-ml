//! CPU-vs-GPU portability test.
//!
//! This test is only compiled when the `gpu` feature is enabled. It runs
//! the full `analyze` pipeline and the `policy()` combiner on both the
//! flex and wgpu devices for a small set of fixed showcase boards
//! and asserts exact equality.
//!
//! Exact equality is valid because every kernel weight and every input
//! plane value is a small integer, so all `f32` sums are exact on both
//! backends.

#![cfg(feature = "gpu")]

use burn::tensor::Device;
use patterns::net::ThreatNet;
use patterns::potential::policy;
use patterns::render::{ShowcaseName, showcase_board, showcase_side};

fn assert_maps_equal_cpu_gpu(showcase: ShowcaseName) {
    let board = showcase_board(showcase);
    let side = showcase_side(showcase);

    let cpu_device = Device::flex();
    let gpu_device = Device::wgpu(Default::default());

    let cpu_net = ThreatNet::new(&cpu_device);
    let gpu_net = ThreatNet::new(&gpu_device);

    let cpu_maps = patterns::net::analyze(&cpu_net, &board, side, &cpu_device);
    let gpu_maps = patterns::net::analyze(&gpu_net, &board, side, &gpu_device);

    assert_eq!(
        cpu_maps, gpu_maps,
        "ThreatMaps must be identical on CPU and GPU for showcase {:?}",
        showcase
    );

    let cpu_policy = policy(&cpu_net, &board, side, &cpu_device);
    let gpu_policy = policy(&gpu_net, &board, side, &gpu_device);

    assert_eq!(
        cpu_policy, gpu_policy,
        "policy() must be identical on CPU and GPU for showcase {:?}",
        showcase
    );
}

#[test]
fn double_three_fork_cpu_gpu_equal() {
    assert_maps_equal_cpu_gpu(ShowcaseName::DoubleThreeFork);
}

#[test]
fn classic_fork_cpu_gpu_equal() {
    assert_maps_equal_cpu_gpu(ShowcaseName::ClassicFork);
}

#[test]
fn corner_fork_cpu_gpu_equal() {
    assert_maps_equal_cpu_gpu(ShowcaseName::CornerFork);
}

#[test]
fn quiet_position_cpu_gpu_equal() {
    assert_maps_equal_cpu_gpu(ShowcaseName::Quiet);
}
