//! Prior inspector: dump what the baseline evaluator's policy actually says
//! about a position — logits, softmaxed priors, the GUI's min-max overlay
//! values, and the threat-map memberships behind every tier.
//!
//! The built-in position is the one that exposed the missing open-three
//! defense tier (T3B):
//! Black: G7 H8 I8 K7 — White: I7 I9 J8 K9, black to move.
//! White's open three is the diagonal I7-J8-K9 (blocks: H6, L10).
//! White also has the row-9 gap I9 _ K9 (fill: J9; ends: H9, L9).
//!
//! Run with: cargo run -p baseline --bin explain_prior

use baseline::{BaselineEvaluator, Device};
use engine::{Board, Move};
use mcts::{EvalRequest, Evaluator};
use patterns::net::ThreatNet;

fn mv(col: u8, row_label: u8) -> Move {
    Move::new(row_label - 1, col - b'A').unwrap()
}

fn label(m: Move) -> String {
    format!("{}{}", (b'A' + m.col()) as char, m.row() + 1)
}

fn main() {
    let mut board = Board::new();
    for m in [
        mv(b'G', 7),
        mv(b'I', 7),
        mv(b'H', 8),
        mv(b'I', 9),
        mv(b'I', 8),
        mv(b'J', 8),
        mv(b'K', 7),
        mv(b'K', 9),
    ] {
        board.play(m).unwrap();
    }

    let device = Device::flex();
    let mut eval = BaselineEvaluator::new(Device::flex());
    let legal: Vec<_> = board.empty_moves().collect();
    let res = eval.evaluate(EvalRequest {
        planes: engine::encode(&board),
        legal: legal.clone(),
    });
    let mut probs = mcts::masked_softmax(&res.policy, &legal);
    probs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    let pmax = probs.first().unwrap().1;
    let pmin = probs.last().unwrap().1;
    let span = pmax - pmin;

    println!(
        "value = {:.4}   pmax = {:.5}   pmin = {:.6}",
        res.value, pmax, pmin
    );
    println!("\n top | point |   logit |    prob | minmax | paints(>0.1)");
    for (rank, (m, p)) in probs.iter().take(12).enumerate() {
        let norm = (p - pmin) / span;
        println!(
            " {:3} | {:>4} | {:7.3} | {:.5} |  {:.3} | {}",
            rank + 1,
            label(*m),
            res.policy[m.index()],
            p,
            norm,
            norm > 0.1
        );
    }

    println!("\n open-three blocks and other named points:");
    for m in [
        mv(b'H', 6),
        mv(b'L', 10),
        mv(b'J', 9),
        mv(b'H', 9),
        mv(b'L', 9),
    ] {
        let p = probs
            .iter()
            .find(|(q, _)| *q == m)
            .map(|(_, p)| *p)
            .unwrap();
        let norm = (p - pmin) / span;
        let rank = probs.iter().position(|(q, _)| *q == m).unwrap() + 1;
        println!(
            " {:3} | {:>4} | {:7.3} | {:.5} |  {:.3} | {}",
            rank,
            label(m),
            res.policy[m.index()],
            p,
            norm,
            norm > 0.1
        );
    }

    let net = ThreatNet::new(&device);
    let maps = patterns::net::analyze_both(&net, &board, &device);
    let own = &maps[0];
    let opp = &maps[1];
    println!("\n threat-map membership (black to move; opp = white):");
    println!(" point | own.win own.dt own.d3 own.3 | opp.win opp.dt opp.d3 opp.3");
    for m in [
        mv(b'H', 6),
        mv(b'L', 10),
        mv(b'J', 9),
        mv(b'H', 9),
        mv(b'L', 9),
        mv(b'J', 10),
    ] {
        let (r, c) = (m.row() as usize, m.col() as usize);
        let any3 =
            |maps: &patterns::net::ThreatMaps| maps.threes_per_dir.iter().any(|d| d[r][c] > 0.0);
        println!(
            " {:>4}  | {:7.1} {:6.1} {:6.1} {:5} | {:7.1} {:6.1} {:6.1} {:5}",
            label(m),
            own.wins[r][c],
            own.double_threats[r][c],
            own.double_threes[r][c],
            any3(own),
            opp.wins[r][c],
            opp.double_threats[r][c],
            opp.double_threes[r][c],
            any3(opp),
        );
    }
}
