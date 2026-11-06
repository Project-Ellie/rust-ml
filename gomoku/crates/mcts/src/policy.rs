//! Move selection from visit counts: temperature and Dirichlet noise.
//!
//! Primer §5: the improved policy π is the root visit-count
//! distribution. From it we either sample (self-play, first 12 moves,
//! τ=1) or play argmax (competitive / after move 12, τ→0). Dirichlet
//! noise is applied to the root priors during self-play only.
//!
use crate::tree::{Edge, NodeId, Tree};
use engine::Move;
use rand::{Rng, RngExt};
use rand_distr::Gamma;

/// Extract the visit count distribution over the root's edges.
///
/// Returns `(Move, visits)` pairs in edge-creation order. Moves with
/// zero visits are included because they are still legal and part of
/// the full policy target.
#[must_use]
pub fn visit_distribution(tree: &Tree, root: NodeId) -> Vec<(Move, u32)> {
    tree.node(root)
        .edges()
        .iter()
        .map(|e| (e.mv, e.n))
        .collect()
}

/// Pick one move from a visit-count distribution using temperature.
///
/// * `temperature < 1e-8`: play the most-visited move (argmax). Ties
///   broken deterministically by lowest [`Move::index`].
/// * `temperature == 1.0`: sample proportional to visits.
/// * General `τ > 0`: sample proportional to `N^(1/τ)`.
///
/// Returns `None` for an empty distribution.
///
/// # Panics
/// Panics only on internal invariants (e.g. a corrupted empty
/// distribution reaching the argmax branch).
#[must_use]
pub fn select_move(dist: &[(Move, u32)], temperature: f32, rng: &mut impl Rng) -> Option<Move> {
    if dist.is_empty() {
        return None;
    }

    if temperature < 1e-8 {
        return Some(
            dist.iter()
                .copied()
                .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.index().cmp(&a.0.index())))
                .map(|(mv, _)| mv)
                .expect("Dist must not be empty"),
        );
    }

    let exponent = 1.0 / f64::from(temperature);

    // Compute weights = N^(1/τ). For N == 0, weight is 0 unless all
    // counts are zero (e.g. before any simulations), in which case we
    // fall back to uniform.
    let mut weights: Vec<f64> = dist
        .iter()
        .map(|(_, n)| f64::from(*n).powf(exponent))
        .collect();

    if weights.iter().all(|&w| w == 0.0) {
        let uniform = 1.0 / weights.len() as f64;
        weights.fill(uniform);
    }

    let total: f64 = weights.iter().sum();
    let threshold = rng.random::<f64>() * total;
    let mut accum = 0.0;
    for (i, &w) in weights.iter().enumerate() {
        accum += w;
        if accum >= threshold {
            return Some(dist[i].0);
        }
    }

    Some(dist[dist.len() - 1].0)
}

/// Pick one move from root edges using temperature, breaking
/// zero-temperature ties by prior.
///
/// * `temperature < 1e-8`: play the most-visited edge. Ties are broken
///   by higher root prior, then by lowest [`Move::index`]. A higher
///   prior never beats a higher visit count. This branch consumes no
///   RNG.
/// * `temperature > 0`: delegates to [`select_move`] on the
///   `(Move, visits)` pairs; priors are ignored and sampling behavior
///   (including the all-zero-counts uniform fallback and RNG
///   consumption) is identical to [`select_move`].
///
/// Returns `None` for empty edges.
#[must_use]
pub fn select_move_with_priors(
    edges: &[Edge],
    temperature: f32,
    rng: &mut impl Rng,
) -> Option<Move> {
    if temperature < 1e-8 {
        return edges
            .iter()
            .max_by(|a, b| {
                a.n.cmp(&b.n)
                    .then_with(|| {
                        a.prior
                            .partial_cmp(&b.prior)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .then_with(|| b.mv.index().cmp(&a.mv.index()))
            })
            .map(|edge| edge.mv);
    }
    let dist: Vec<(Move, u32)> = edges.iter().map(|edge| (edge.mv, edge.n)).collect();
    select_move(&dist, temperature, rng)
}

/// Add Dirichlet noise to the priors of the root edges only.
///
/// `P'(a) = (1 - ε) * P(a) + ε * η_a`, where `η ~ Dirichlet(α)` over
/// the root edges. Interior nodes are untouched (primer §5).
///
/// # Panics
/// Panics if the root has no edges.
pub fn add_dirichlet_noise(
    tree: &mut Tree,
    root: NodeId,
    epsilon: f32,
    alpha: f32,
    rng: &mut impl Rng,
) {
    let edges = tree.node(root).edges();
    let k = edges.len();
    assert!(k > 0, "cannot add Dirichlet noise to a root with no edges");

    // Dirichlet(α) over k categories: sample k independent
    // Gamma(α, 1) variables and normalize. rand_distr 0.5's
    // `Dirichlet` requires a const-size array, so we use Gamma directly.
    let shape = f64::from(alpha);
    let gamma = Gamma::new(shape, 1.0).expect("valid Gamma parameters");
    let mut noise: Vec<f64> = (0..k).map(|_| rng.sample(gamma)).collect();
    let noise_sum: f64 = noise.iter().sum();
    if noise_sum > 0.0 {
        for v in &mut noise {
            *v /= noise_sum;
        }
    } else {
        // Astronomically unlikely for α > 0; fall back to uniform.
        let uniform = 1.0 / k as f64;
        noise.fill(uniform);
    }

    let eps = f64::from(epsilon);
    let one_minus_eps = 1.0 - eps;
    let edges = tree.node_mut(root).edges_mut();
    for (edge, eta) in edges.iter_mut().zip(noise) {
        let prior = f64::from(edge.prior);
        edge.prior = (one_minus_eps * prior + eps * eta) as f32;
    }

    // Renormalize to protect against tiny rounding drift.
    let sum: f32 = tree.node(root).edges().iter().map(|e| e.prior).sum();
    if sum > 0.0 {
        for edge in tree.node_mut(root).edges_mut() {
            edge.prior /= sum;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::Edge;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    fn edge(mv: (u8, u8), n: u32) -> Edge {
        edge_with(mv, 0.1, n)
    }

    fn edge_with(mv: (u8, u8), prior: f32, n: u32) -> Edge {
        Edge {
            mv: Move::new(mv.0, mv.1).unwrap(),
            prior,
            n,
            w: 0.0,
            child: None,
        }
    }

    #[test]
    fn visit_distribution_matches_edges() {
        let mut tree = Tree::new();
        tree.node_mut(0).edges_mut().push(edge((7, 7), 5));
        tree.node_mut(0).edges_mut().push(edge((7, 8), 3));
        let dist = visit_distribution(&tree, 0);
        assert_eq!(
            dist,
            vec![(Move::new(7, 7).unwrap(), 5), (Move::new(7, 8).unwrap(), 3)]
        );
    }

    #[test]
    fn argmax_picks_most_visited() {
        let dist = vec![
            (Move::new(0, 0).unwrap(), 1),
            (Move::new(7, 7).unwrap(), 50),
            (Move::new(0, 1).unwrap(), 5),
        ];
        let mut rng = StdRng::seed_from_u64(1);
        let mv = select_move(&dist, 0.0, &mut rng).unwrap();
        assert_eq!(mv, Move::new(7, 7).unwrap());
    }

    #[test]
    fn argmax_tie_breaks_by_lowest_index() {
        let dist = vec![
            (Move::new(0, 0).unwrap(), 5),
            (Move::new(7, 7).unwrap(), 5),
            (Move::new(0, 1).unwrap(), 5),
        ];
        let mut rng = StdRng::seed_from_u64(1);
        let mv = select_move(&dist, 0.0, &mut rng).unwrap();
        assert_eq!(mv, Move::new(0, 0).unwrap());
    }

    #[test]
    fn select_move_empty_returns_none() {
        let dist: &[(Move, u32)] = &[];
        let mut rng = StdRng::seed_from_u64(1);
        assert_eq!(select_move(dist, 0.0, &mut rng), None);
    }

    #[test]
    fn argmax_more_visits_beat_a_higher_prior() {
        let edges = vec![edge_with((7, 7), 0.9, 1), edge_with((3, 3), 0.01, 5)];
        let mut rng = StdRng::seed_from_u64(1);
        let mv = select_move_with_priors(&edges, 0.0, &mut rng).unwrap();
        assert_eq!(mv, Move::new(3, 3).unwrap());
    }

    #[test]
    fn argmax_tied_visits_pick_the_higher_prior() {
        let edges = vec![
            edge_with((0, 0), 0.01, 5),
            edge_with((7, 7), 0.9, 5),
            edge_with((3, 3), 0.05, 5),
        ];
        let mut rng = StdRng::seed_from_u64(1);
        let mv = select_move_with_priors(&edges, 0.0, &mut rng).unwrap();
        assert_eq!(mv, Move::new(7, 7).unwrap());
    }

    #[test]
    fn argmax_tied_visits_and_priors_pick_the_lowest_index_in_any_order() {
        let forward = vec![
            edge_with((7, 7), 0.5, 5),
            edge_with((0, 0), 0.5, 5),
            edge_with((3, 3), 0.5, 1),
        ];
        let reversed: Vec<_> = forward.iter().rev().cloned().collect();
        for edges in [forward, reversed] {
            let mut rng = StdRng::seed_from_u64(1);
            let mv = select_move_with_priors(&edges, 0.0, &mut rng).unwrap();
            assert_eq!(mv, Move::new(0, 0).unwrap());
        }
    }

    #[test]
    fn zero_temperature_consumes_no_rng() {
        let edges = vec![edge_with((0, 0), 0.4, 0), edge_with((7, 7), 0.6, 0)];
        let mut rng = StdRng::seed_from_u64(5);
        let mut untouched = StdRng::seed_from_u64(5);
        let mv = select_move_with_priors(&edges, 0.0, &mut rng).unwrap();
        assert_eq!(mv, Move::new(7, 7).unwrap());
        assert_eq!(rng.random::<u64>(), untouched.random::<u64>());
    }

    #[test]
    fn positive_temperature_matches_counts_only_selection() {
        let edges = vec![
            edge_with((0, 0), 0.9, 1),
            edge_with((7, 7), 0.05, 9),
            edge_with((3, 3), 0.05, 4),
        ];
        let dist: Vec<(Move, u32)> = edges.iter().map(|e| (e.mv, e.n)).collect();
        for temperature in [1.0, 0.7] {
            let mut edge_rng = StdRng::seed_from_u64(42);
            let mut legacy_rng = StdRng::seed_from_u64(42);
            for _ in 0..20 {
                let via_edges =
                    select_move_with_priors(&edges, temperature, &mut edge_rng).unwrap();
                let via_counts = select_move(&dist, temperature, &mut legacy_rng).unwrap();
                assert_eq!(via_edges, via_counts);
            }
            assert_eq!(edge_rng.random::<u64>(), legacy_rng.random::<u64>());
        }
    }

    #[test]
    fn positive_temperature_all_zero_counts_falls_back_like_legacy() {
        let edges = vec![
            edge_with((0, 0), 0.9, 0),
            edge_with((7, 7), 0.05, 0),
            edge_with((3, 3), 0.05, 0),
        ];
        let dist: Vec<(Move, u32)> = edges.iter().map(|e| (e.mv, e.n)).collect();
        let mut edge_rng = StdRng::seed_from_u64(9);
        let mut legacy_rng = StdRng::seed_from_u64(9);
        for _ in 0..20 {
            let via_edges = select_move_with_priors(&edges, 1.0, &mut edge_rng).unwrap();
            let via_counts = select_move(&dist, 1.0, &mut legacy_rng).unwrap();
            assert_eq!(via_edges, via_counts);
        }
        assert_eq!(edge_rng.random::<u64>(), legacy_rng.random::<u64>());
    }

    #[test]
    fn empty_edges_return_none() {
        let edges: &[Edge] = &[];
        let mut rng = StdRng::seed_from_u64(1);
        assert_eq!(select_move_with_priors(edges, 0.0, &mut rng), None);
        assert_eq!(select_move_with_priors(edges, 1.0, &mut rng), None);
    }

    #[test]
    fn tau_one_sampling_approximately_matches_distribution() {
        // Visit counts 9:1. With τ=1, sampling should heavily favor
        // the first move. We use a seeded RNG and a loose bound.
        let dist = vec![
            (Move::new(0, 0).unwrap(), 90),
            (Move::new(0, 1).unwrap(), 10),
        ];
        let mut rng = StdRng::seed_from_u64(42);
        let mut first = 0usize;
        let trials = 1000;
        for _ in 0..trials {
            let mv = select_move(&dist, 1.0, &mut rng).unwrap();
            if mv == Move::new(0, 0).unwrap() {
                first += 1;
            }
        }
        assert!(first > 700 && first < 980, "first = {first}/1000");
    }

    #[test]
    fn dirichlet_noise_keeps_sum_one_and_touches_only_root() {
        let mut tree = Tree::new();
        for &(r, c) in &[(7, 7), (7, 8), (7, 9)] {
            tree.node_mut(0).edges_mut().push(edge((r, c), 0));
        }
        // Add an interior node with edges so we can check it is untouched.
        let child = tree.add_child(0, 0);
        tree.node_mut(child).edges_mut().push(edge((6, 6), 0));
        let interior_prior = tree.node(child).edges()[0].prior;

        let mut rng = StdRng::seed_from_u64(7);
        add_dirichlet_noise(&mut tree, 0, 0.25, 0.1, &mut rng);

        let sum: f32 = tree.node(0).edges().iter().map(|e| e.prior).sum();
        assert!((sum - 1.0).abs() < 1e-4, "sum = {sum}");
        assert!(tree.node(0).edges().iter().all(|e| e.prior > 0.0));
        assert_eq!(tree.node(child).edges()[0].prior, interior_prior);

        // Seeded reproducibility.
        let mut tree2 = Tree::new();
        for &(r, c) in &[(7, 7), (7, 8), (7, 9)] {
            tree2.node_mut(0).edges_mut().push(edge((r, c), 0));
        }
        let mut rng2 = StdRng::seed_from_u64(7);
        add_dirichlet_noise(&mut tree2, 0, 0.25, 0.1, &mut rng2);
        for (a, b) in tree
            .node(0)
            .edges()
            .iter()
            .zip(tree2.node(0).edges().iter())
        {
            assert!((a.prior - b.prior).abs() < 1e-7);
        }
    }
}
