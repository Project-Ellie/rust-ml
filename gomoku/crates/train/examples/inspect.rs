//! Print 10 evenly-spaced samples from the synthetic dataset.

use engine::{Color, Move};
use std::path::Path;
use train::dataset::read_dataset;
use train::label::{TacticalClass, classify};

fn render(sample: &train::sample::Sample, target: Move) -> String {
    let board = sample.board().unwrap();
    let mut out = String::new();
    out.push_str("   a b c d e f g h i j k l m n o\n");
    for r in 0..15u8 {
        out.push_str(&format!("{r:2} "));
        for c in 0..15u8 {
            let mv = Move::new(r, c).unwrap();
            let ch = match board.stone_at(mv) {
                Some(Color::Black) => 'X',
                Some(Color::White) => 'O',
                None if mv == target => '*',
                None => '.',
            };
            out.push(ch);
            out.push(' ');
        }
        out.push('\n');
    }
    out
}

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data/synthetic/v1");
    let samples = read_dataset(&dir).unwrap();
    println!(
        "Dataset: {} samples from {}\n",
        samples.len(),
        dir.display()
    );

    let step = samples.len() / 10;
    for i in 0..10 {
        let idx = i * step;
        let s = &samples[idx];
        let class = classify(&s.board().unwrap());
        let (target, p) = s
            .policy
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .copied()
            .unwrap();
        let class_str = match class {
            TacticalClass::Win => "WIN (immediate win available)",
            TacticalClass::Block => "BLOCK (opponent threat must be blocked)",
            TacticalClass::Quiet => "QUIET (no forcing move)",
        };
        println!("=== sample #{idx} === class: {class_str}");
        println!(
            "  to_move: {:?} | stones: {} black, {} white | value target: {:+.1}",
            s.to_move,
            s.black.len(),
            s.white.len(),
            s.value
        );
        println!(
            "  policy: {} entries | argmax: ({},{}) with p = {p:.4}",
            s.policy.len(),
            target.row(),
            target.col()
        );
        print!("{}", render(s, target));
        println!();
    }
}
