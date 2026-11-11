//! Stratified evaluation of a trained checkpoint on the holdout set.
//!
//! Usage: eval_stratified <checkpoint-dir>
//!
//! Reports policy top-1 and value-sign accuracy overall, per tactical
//! class, and per board-clutter bucket. Uses `model.valid()` so
//! BatchNorm runs on its learned statistics.

use burn::data::dataloader::batcher::Batcher;
use burn::module::Module;
use burn::tensor::Device;
use net::batcher::NetBatcher;
use net::checkpoint::load_model;
use net::network::ModelConfig;
use std::path::{Path, PathBuf};
use train::dataset::read_dataset;
use train::label::{TacticalClass, classify};
use train::split::is_holdout;

#[derive(Default, Clone, Copy)]
struct Bucket {
    n: u64,
    top1: u64,
    vsign: u64,
}

impl Bucket {
    fn record(&mut self, top1_ok: bool, vsign_ok: bool) {
        self.n += 1;
        self.top1 += top1_ok as u64;
        self.vsign += vsign_ok as u64;
    }
    fn line(&self, name: &str) -> String {
        if self.n == 0 {
            return format!("{name:<28} n=0");
        }
        format!(
            "{name:<28} n={:5} top1={:6.2}% vsign={:6.2}%",
            self.n,
            100.0 * self.top1 as f64 / self.n as f64,
            100.0 * self.vsign as f64 / self.n as f64
        )
    }
}

fn main() {
    let ckpt = PathBuf::from(std::env::args().nth(1).expect("checkpoint dir"));
    let device = Device::wgpu(Default::default());
    let model = load_model(&ckpt, &ModelConfig::mid(), &device)
        .expect("load checkpoint")
        .valid();

    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data/synthetic/v1");
    let samples = read_dataset(&dir).unwrap();
    let holdout: Vec<_> = samples.into_iter().filter(is_holdout).collect();
    println!("holdout: {} samples", holdout.len());

    let batcher = NetBatcher::holdout();
    let mut overall = Bucket::default();
    let mut win = Bucket::default();
    let mut block = Bucket::default();
    let mut quiet = Bucket::default();
    let mut few = Bucket::default(); // <= 20 stones
    let mut mid = Bucket::default(); // 21..=40
    let mut many = Bucket::default(); // > 40 stones

    for chunk in holdout.chunks(256) {
        let classes: Vec<TacticalClass> = chunk
            .iter()
            .map(|s| classify(&s.board().unwrap()))
            .collect();
        let stones: Vec<usize> = chunk
            .iter()
            .map(|s| s.black.len() + s.white.len())
            .collect();

        let batch = batcher.batch(chunk.to_vec(), &device);
        let out = model.forward(batch.input);
        let logits: Vec<f32> = out.policy.into_data().try_to_vec::<f32>().unwrap();
        let values: Vec<f32> = out.value.into_data().try_to_vec::<f32>().unwrap();
        let targets: Vec<f32> = batch.policy_target.into_data().try_to_vec::<f32>().unwrap();

        for (i, s) in chunk.iter().enumerate() {
            let row = &logits[i * 225..(i + 1) * 225];
            let pred = row
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                .map(|(i, _)| i)
                .unwrap();
            let trow = &targets[i * 225..(i + 1) * 225];
            let tmax = trow
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                .map(|(i, _)| i)
                .unwrap();
            // For multi-move tactical targets any max-mass move counts.
            let top1_ok = trow[pred] >= trow[tmax] - 1e-6 && trow[pred] > 0.0;
            let vsign_ok = (values[i] > 0.0) == (s.value > 0.0);

            overall.record(top1_ok, vsign_ok);
            match classes[i] {
                TacticalClass::Win => win.record(top1_ok, vsign_ok),
                TacticalClass::Block => block.record(top1_ok, vsign_ok),
                TacticalClass::Quiet => quiet.record(top1_ok, vsign_ok),
            }
            match stones[i] {
                0..=20 => few.record(top1_ok, vsign_ok),
                21..=40 => mid.record(top1_ok, vsign_ok),
                _ => many.record(top1_ok, vsign_ok),
            }
        }
    }

    println!("{}", overall.line("overall"));
    println!("{}", win.line("class win"));
    println!("{}", block.line("class block"));
    println!("{}", quiet.line("class quiet"));
    println!("{}", few.line("clutter <=20 stones"));
    println!("{}", mid.line("clutter 21-40 stones"));
    println!("{}", many.line("clutter >40 stones"));
}
