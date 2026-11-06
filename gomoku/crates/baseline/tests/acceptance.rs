//! Acceptance tests for the baseline evaluator (sub-chapter 09).
//!
//! Tactical soundness is asserted on every run. Strength is measured by
//! the ignored `strength_ladder_measured` test; interactive latency by
//! the ignored `interactive_latency_measured`. The quiet-whisper A/B
//! from sub-chapter 07 is preserved as `quiet_whisper_ab_measured` —
//! a raw-search, probe-free evaluator-isolation experiment kept for
//! historical comparability (its recorded 20/20 both arms, 23.5 vs 34.5
//! average plies predate the player-level proof path). It is NOT a
//! full-seat measurement: the production seat (`player::AiPlayer` with
//! the own TSS probe and the opponent-proof guard) is what the strength
//! ladder measures.
//!
//! All player-level games are driven through one persistent
//! `player::AiPlayer` per seat, with the exact CLI policies: the
//! baseline seat runs `OwnTssConfig::baseline()` (10,000 nodes / 30
//! plies) plus `GuardConfig::baseline()` (5 candidates, 1,000 nodes /
//! 10 plies), the tactics seat runs both disabled (see
//! `cli::session`'s per-`EvaluatorKind` mapping).

use baseline::{BaselineEvaluator, Device, adapter};
use engine::{Board, Color, Move, Status, reference::board_from_ascii};
use mcts::{EvalRequest, EvalResult, Evaluator, TacticsEvaluator};
use player::{AiPlayer, Decision, GuardConfig, OwnTssConfig, PlayerOptions};
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};
use std::io::Write;
use std::time::Instant;

/// Base seed for every measurement in this file (pre-registered).
const BASE_SEED: u64 = 20_260_801;

/// Flush stdout so measurement progress is visible as it happens.
fn flush() {
    std::io::stdout().flush().expect("stdout flushes");
}

// ------------------------------------------------------------------
// Puzzle pack parsing — deliberate minimal duplication of
// `cli::puzzle::PuzzlePack` to avoid a dev-dependency cycle (cli
// already depends on baseline).
// ------------------------------------------------------------------

mod pack {
    use engine::{Board, Color, Move, PositionError, Status};
    use serde::Deserialize;
    use std::collections::HashSet;
    use std::path::PathBuf;

    /// One parsed puzzle position.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Puzzle {
        /// Zero-based index within the pack.
        pub index: usize,
        /// Side to move (the attacker).
        pub attacker: Color,
        /// The board position with the attacker to move.
        pub board: Board,
        /// Claimed forced-win line, `(move, side)` per ply.
        pub solution: Vec<(Move, Color)>,
    }

    /// A parsed puzzle pack.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct PuzzlePack {
        /// All puzzles in file order.
        pub items: Vec<Puzzle>,
    }

    impl PuzzlePack {
        /// Parse a pack from JSON text.
        ///
        /// # Errors
        /// Returns a string describing the first parsing or validation
        /// problem.
        pub fn from_json(text: &str) -> Result<Self, String> {
            let file: PackFile = serde_json::from_str(text).map_err(|e| e.to_string())?;
            if file.count != file.items.len() {
                return Err(format!(
                    "count mismatch: declared {}, actual {}",
                    file.count,
                    file.items.len()
                ));
            }
            let items = file
                .items
                .into_iter()
                .enumerate()
                .map(|(pos, item)| parse_item(pos, item))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Self { items })
        }

        /// Load a pack from a path relative to this crate's manifest dir.
        pub fn from_path(path: &str) -> Self {
            let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let full = manifest.join(path);
            let text = std::fs::read_to_string(&full)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", full.display()));
            Self::from_json(&text).unwrap_or_else(|e| panic!("invalid {}: {e}", full.display()))
        }
    }

    #[derive(Debug, Deserialize)]
    struct PackFile {
        count: usize,
        items: Vec<PackItem>,
    }

    #[derive(Debug, Deserialize)]
    struct PackItem {
        i: u32,
        a: String,
        #[serde(rename = "b")]
        black: Vec<[u8; 2]>,
        #[serde(rename = "w")]
        white: Vec<[u8; 2]>,
        #[serde(rename = "l", default)]
        line: Vec<[u8; 3]>,
    }

    fn parse_item(pos: usize, item: PackItem) -> Result<Puzzle, String> {
        let attacker = match item.a.as_str() {
            "black" => Color::Black,
            "white" => Color::White,
            other => return Err(format!("puzzle {} invalid attacker {other}", item.i)),
        };
        let black = parse_stones(item.i, &item.black)?;
        let white = parse_stones(item.i, &item.white)?;

        let mut black_seen = HashSet::new();
        for mv in &black {
            if !black_seen.insert(mv.index()) {
                return Err(format!("puzzle {} duplicate black stone", item.i));
            }
        }
        let mut white_seen = HashSet::new();
        for mv in &white {
            if !white_seen.insert(mv.index()) {
                return Err(format!("puzzle {} duplicate white stone", item.i));
            }
            if black_seen.contains(&mv.index()) {
                return Err(format!("puzzle {} overlapping colors", item.i));
            }
        }

        let board = Board::from_position(&black, &white, attacker).map_err(|e| match e {
            PositionError::InvalidCounts => format!(
                "puzzle {} invalid counts (black={}, white={}, attacker={attacker:?})",
                item.i,
                black.len(),
                white.len()
            ),
            PositionError::OverlappingColors => {
                format!("puzzle {} overlapping colors", item.i)
            }
        })?;
        if board.status() != Status::Ongoing {
            return Err(format!("puzzle {} board is already decided", item.i));
        }

        let mut solution = Vec::with_capacity(item.line.len());
        for (ply, &[x, y, side]) in item.line.iter().enumerate() {
            let color = match side {
                1 => Color::Black,
                2 => Color::White,
                other => return Err(format!("puzzle {} invalid side {other}", item.i)),
            };
            let expected = if ply % 2 == 0 {
                attacker
            } else {
                attacker.other()
            };
            if color != expected {
                return Err(format!(
                    "puzzle {} line ply {ply} is {color:?}, expected {expected:?}",
                    item.i
                ));
            }
            let mv =
                Move::new(y, x).ok_or(format!("puzzle {} coord ({x},{y}) off board", item.i))?;
            solution.push((mv, color));
        }

        Ok(Puzzle {
            index: pos,
            attacker,
            board,
            solution,
        })
    }

    fn parse_stones(item: u32, coords: &[[u8; 2]]) -> Result<Vec<Move>, String> {
        coords
            .iter()
            .map(|&[x, y]| {
                Move::new(y, x).ok_or(format!("puzzle {item} coord ({x},{y}) off board"))
            })
            .collect()
    }
}

use pack::{Puzzle, PuzzlePack};

// ------------------------------------------------------------------
// Drivers
// ------------------------------------------------------------------

/// The brain seated at one side.
enum Brain {
    /// Uniform random legal moves.
    Random,
    /// Baseline evaluator with the given simulation budget.
    Baseline(u32),
    /// Tactics evaluator with the given simulation budget.
    Tactics(u32),
}

/// One seat's persistent player, built once per game — constructing
/// the `ThreatNet` per move would multiply the cost, and the player's
/// whole point is one persistent evaluator per seat.
enum Seat {
    /// Uniform random legal moves, no evaluator.
    Random,
    /// Persistent baseline player.
    Baseline(AiPlayer),
    /// Persistent tactics player.
    Tactics(AiPlayer),
}

impl Brain {
    fn seat(&self) -> Seat {
        match self {
            Brain::Random => Seat::Random,
            Brain::Baseline(_) => Seat::Baseline(AiPlayer::new(Box::new(BaselineEvaluator::new(
                Device::flex(),
            )))),
            Brain::Tactics(_) => Seat::Tactics(AiPlayer::new(Box::new(TacticsEvaluator::new()))),
        }
    }
}

/// The baseline seat's options: the exact CLI policy for
/// `EvaluatorKind::Baseline` — own TSS probe (10,000 nodes / 30 plies)
/// and opponent-proof guard (5 candidates, 1,000 nodes / 10 plies).
fn baseline_options(simulations: u32) -> PlayerOptions {
    PlayerOptions {
        simulations,
        c_puct: 1.5,
        own: OwnTssConfig::baseline(),
        guard: GuardConfig::baseline(),
        ..PlayerOptions::default()
    }
}

/// The tactics seat's options: the exact CLI policy for
/// `EvaluatorKind::Tactics` — probe and guard disabled.
fn tactics_options(simulations: u32) -> PlayerOptions {
    PlayerOptions {
        simulations,
        c_puct: 1.5,
        ..PlayerOptions::default()
    }
}

/// A uniform random legal move.
fn random_move(board: &Board, rng: &mut StdRng) -> Move {
    let legal: Vec<Move> = board.empty_moves().collect();
    legal[rng.random_range(0..legal.len())]
}

/// Player-level move helper.
///
/// The own TSS probe, the single `mcts::search`, and the
/// opponent-proof guard all run inside `decide`. A certified own
/// proof returns its move directly, with no search record and zero
/// RNG draws. Never call `engine::prove_forced_win` from this harness
/// — the player is the certifier.
fn best_move(
    player: &mut AiPlayer,
    board: &Board,
    options: &PlayerOptions,
    rng: &mut StdRng,
) -> Move {
    player.decide(board, options, rng).expect("game ongoing").mv
}

/// Per-search configured-budget metrics for the baseline seat.
///
/// * `searches` — how many times the baseline was asked to move.
/// * `requested_sims` — sum of configured `simulations` arguments
///   passed to those decisions. This is intentionally a budget-rate
///   metric, NOT a count of performed leaf evaluations. A certified
///   own TSS proof credits the full configured budget while running
///   zero simulations.
/// * `actual_sims` — sum of root visit totals across the decisions
///   that actually searched (each simulation visits exactly one root
///   edge). Zero for proof-bypassed decisions.
/// * `proofs` — decisions certified by the own TSS probe (search
///   bypassed entirely).
/// * `search_time_secs` — wall-clock time inside the baseline's move
///   decisions, including the root TSS probe and all root overhead.
///
/// Therefore `requested_sims / search_time_secs` is a
/// configured-budget rate only; it is neither a lower bound nor a
/// measure of actual leaf-evaluation throughput (never "evals/sec").
#[derive(Debug, Default)]
struct BaselineMetrics {
    searches: u64,
    requested_sims: u64,
    actual_sims: u64,
    proofs: u64,
    search_time_secs: f64,
}

impl BaselineMetrics {
    fn record(&mut self, decision: &Decision, requested: u32, secs: f64) {
        self.searches += 1;
        self.requested_sims += u64::from(requested);
        self.search_time_secs += secs;
        match &decision.search {
            Some(stats) => {
                self.actual_sims += stats.visits.iter().map(|(_, n)| u64::from(*n)).sum::<u64>();
            }
            None => self.proofs += 1,
        }
    }
}

/// Play one game to completion (or the 225-ply cap), asserting every
/// move is legal. Wins are read from the board's terminal status —
/// never from line agreement — so no arm of the harness can "win" by
/// construction rather than by play.
///
/// Returns `(status, plies, baseline_metrics)`.
fn play_game(black: Brain, white: Brain, seed: u64) -> (Status, u32, BaselineMetrics) {
    let mut board = Board::new();
    let mut rng = StdRng::seed_from_u64(seed);
    let mut plies = 0u32;
    let mut black_seat = black.seat();
    let mut white_seat = white.seat();
    let mut metrics = BaselineMetrics::default();

    while board.status() == Status::Ongoing && plies < 225 {
        let side = board.to_move();
        let (brain, seat) = match side {
            Color::Black => (&black, &mut black_seat),
            Color::White => (&white, &mut white_seat),
        };
        let mv = match (brain, seat) {
            (Brain::Random, Seat::Random) => random_move(&board, &mut rng),
            (Brain::Baseline(sims), Seat::Baseline(player)) => {
                let options = baseline_options(*sims);
                let start = Instant::now();
                let decision = player
                    .decide(&board, &options, &mut rng)
                    .expect("game ongoing");
                metrics.record(&decision, *sims, start.elapsed().as_secs_f64());
                decision.mv
            }
            (Brain::Tactics(sims), Seat::Tactics(player)) => {
                best_move(player, &board, &tactics_options(*sims), &mut rng)
            }
            _ => unreachable!("seat kind matches brain kind by construction"),
        };
        assert!(
            board.is_legal(mv),
            "illegal move {mv:?} at ply {plies} for {side:?}"
        );
        board.play(mv).unwrap();
        plies += 1;
    }

    (board.status(), plies, metrics)
}

// ------------------------------------------------------------------
// Puzzle solver
// ------------------------------------------------------------------

/// Generic puzzle solver: the attacker moves through `player` (an
/// `AiPlayer` over any evaluator, with the caller's options), the
/// defender follows the pack's claimed line indexed by ply.
///
/// The defender branch indexes `solution.get(ply)` guarded by side —
/// successive defender replies advance through the line. Do NOT use
/// `find`, which replays the first same-color move forever.
///
/// Returns `(solved, defender_replies)`.
fn solve_puzzle_with(
    puzzle: &Puzzle,
    player: &mut AiPlayer,
    options: &PlayerOptions,
    rng: &mut StdRng,
) -> (bool, Vec<Move>) {
    let mut board = puzzle.board.clone();
    let mut defender_replies = Vec::new();
    let max_plies = puzzle.solution.len() as u32 + 4;
    let attacker = puzzle.attacker;

    for ply in 0..max_plies {
        match board.status() {
            Status::Won(c) if c == attacker => return (true, defender_replies),
            Status::Won(_) | Status::Draw => return (false, defender_replies),
            Status::Ongoing => {}
        }

        let side = board.to_move();
        let mv = if side == attacker {
            best_move(player, &board, options, rng)
        } else {
            // Follow the pack's claimed defender move for this ply.
            // The solution is indexed by ply from the puzzle root; the
            // color guard keeps us from replaying an attacker move when
            // the line is shorter than expected.
            let solution_mv = puzzle
                .solution
                .get(ply as usize)
                .filter(|(_, c)| *c == side)
                .map(|(mv, _)| *mv);
            solution_mv.unwrap_or_else(|| random_move(&board, rng))
        };

        if side != attacker {
            defender_replies.push(mv);
        }

        if !board.is_legal(mv) {
            return (false, defender_replies);
        }
        board.play(mv).unwrap();
    }

    let solved = matches!(board.status(), Status::Won(c) if c == attacker);
    (solved, defender_replies)
}

/// Solve one puzzle with the production baseline seat: a fresh
/// `BaselineEvaluator` in an `AiPlayer` with the full baseline policy,
/// so the own TSS probe fires on the puzzle root inside `decide`.
fn solve_puzzle(puzzle: &Puzzle, simulations: u32, seed: u64) -> bool {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut player = AiPlayer::new(Box::new(BaselineEvaluator::new(Device::flex())));
    solve_puzzle_with(
        puzzle,
        &mut player,
        &baseline_options(simulations),
        &mut rng,
    )
    .0
}

// ------------------------------------------------------------------
// Tactical asserts
// ------------------------------------------------------------------

#[test]
fn win_in_one_is_solved_at_50_sims() {
    // Black has four in a row on row 7. Fifty simulations is
    // deliberately small: the trust ladder's top tiers are exactly the
    // moves PUCT explores first (or the own probe certifies outright),
    // so the right move is found almost immediately, not eventually.
    let b = board_from_ascii(
        "
        O . O . O . O . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . X X X X . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        . . . . . . . . . . . . . . .
        ",
    );
    assert_eq!(b.to_move(), Color::Black);

    let mut rng = StdRng::seed_from_u64(1);
    let mut player = AiPlayer::new(Box::new(BaselineEvaluator::new(Device::flex())));
    let mv = best_move(&mut player, &b, &baseline_options(50), &mut rng);
    let wins = engine::immediate_wins(&b, Color::Black);
    assert!(
        wins.contains(mv),
        "best move {mv:?} must be an immediate win"
    );
}

#[test]
fn forced_block_is_taken_at_50_sims() {
    // Script eight plies that leave White exactly one winning cell.
    let mut b = Board::new();
    let script = [
        (7, 2),
        (7, 3),
        (0, 0),
        (7, 4),
        (0, 1),
        (7, 5),
        (0, 2),
        (7, 6),
    ];
    for &(r, c) in &script {
        b.play(Move::new(r, c).unwrap()).unwrap();
    }
    assert_eq!(b.to_move(), Color::Black);
    let blocks = engine::forced_blocks(&b);
    assert_eq!(blocks.len(), 1, "White must have exactly one winning cell");

    let mut rng = StdRng::seed_from_u64(2);
    let mut player = AiPlayer::new(Box::new(BaselineEvaluator::new(Device::flex())));
    let mv = best_move(&mut player, &b, &baseline_options(50), &mut rng);
    assert!(
        blocks.contains(mv),
        "best move {mv:?} must be the forced block"
    );
}

// ------------------------------------------------------------------
// Self-play flood
// ------------------------------------------------------------------

#[test]
fn fifty_seeded_self_play_games_are_legal() {
    // Generic MCTS health flood: exercises the search/evaluator
    // plumbing for 50 full games without panics. The baseline
    // evaluator costs milliseconds per call (a real convolution
    // forward), which makes a 50-game debug-mode flood minutes slow,
    // while the plumbing the flood exercises — search, backup,
    // legality — is evaluator-agnostic. So the flood deliberately
    // seats the fast TacticsEvaluator at 1 simulation; baseline
    // behavior is covered by the tactical asserts above and the
    // release-only strength ladder measurement below. Record the
    // trade-off; never pay it silently.
    let mut rng = StdRng::seed_from_u64(2026);
    for game in 0..50 {
        let seed = rng.random::<u64>();
        let (status, plies, _) = play_game(Brain::Tactics(1), Brain::Tactics(1), seed);
        assert!(
            status != Status::Ongoing || plies == 225,
            "game {game} did not terminate"
        );
        assert!(plies <= 225, "game {game} exceeded the maximal move count");
    }
}

// ------------------------------------------------------------------
// Pack parser sanity
//
// Provenance honesty: kaibao-vcf-676 and the four renju-portal shards
// (3,808 puzzles total) are engine-proven conversions; vcf-material-763
// is source-claimed, pending milestone-4 re-verification. The parser
// validates format and line alternation; the claimed lines themselves
// are trusted as claimed, not re-proven here.
// ------------------------------------------------------------------

#[test]
fn kaibao_pack_parses_with_expected_count() {
    let pack = PuzzlePack::from_path("../../../data/puzzles/kaibao-vcf-676.json");
    assert_eq!(pack.items.len(), 676);
    assert!(pack.items.iter().all(|p| !p.solution.is_empty()));
}

#[test]
fn all_vcf_packs_parse_and_have_nonempty_lines() {
    let packs = [
        // Engine-proven conversions (3,808 total).
        ("kaibao-vcf-676.json", 676usize),
        ("renju-portal-vcf-783-1of4.json", 783),
        ("renju-portal-vcf-783-2of4.json", 783),
        ("renju-portal-vcf-783-3of4.json", 783),
        ("renju-portal-vcf-783-4of4.json", 783),
        // Source-claimed, pending milestone-4 re-verification.
        ("vcf-material-763.json", 763),
    ];
    for (name, expected) in packs {
        let pack = PuzzlePack::from_path(&format!("../../../data/puzzles/{name}"));
        assert_eq!(
            pack.items.len(),
            expected,
            "{name} should contain {expected} puzzles"
        );
        assert!(
            pack.items.iter().all(|p| !p.solution.is_empty()),
            "{name} contains puzzles with empty solution lines"
        );
    }
}

// ------------------------------------------------------------------
// Successive-defender-replies regression
// ------------------------------------------------------------------

/// Deterministic evaluator that forces the attacker to follow the
/// puzzle's claimed solution line. Used only to make the
/// defender-reply regression independent of baseline search
/// variability. Owns its copy of the line: `AiPlayer` boxes its
/// evaluator as `'static`.
struct SolutionFollowingEvaluator {
    attacker: Color,
    solution: Vec<(Move, Color)>,
    puzzle_empty: usize,
}

impl SolutionFollowingEvaluator {
    fn new(puzzle: &Puzzle) -> Self {
        Self {
            attacker: puzzle.attacker,
            solution: puzzle.solution.clone(),
            puzzle_empty: puzzle.board.empty_moves().count(),
        }
    }

    fn policy_for(&self, req: &EvalRequest) -> EvalResult {
        let board = adapter::reconstruct_board(&req.planes, &req.legal);
        let side = board.to_move();
        let ply = self.puzzle_empty.saturating_sub(req.legal.len());

        let mut policy = [f32::NEG_INFINITY; 225];
        if side == self.attacker
            && let Some((target, color)) = self.solution.get(ply)
            && *color == side
            && req.legal.contains(target)
        {
            policy[target.index()] = 1.0;
            return EvalResult { policy, value: 0.0 };
        }
        for mv in &req.legal {
            policy[mv.index()] = 0.0;
        }
        EvalResult { policy, value: 0.0 }
    }
}

impl Evaluator for SolutionFollowingEvaluator {
    fn evaluate(&mut self, req: EvalRequest) -> EvalResult {
        self.policy_for(&req)
    }
}

#[test]
fn solve_puzzle_follows_successive_defender_replies() {
    // Regression for the chapter-09 restore bug: the defender branch
    // used `find(|(_, c)| c == side)`, which replayed the *first*
    // defender move forever. This puzzle's solution line contains
    // three distinct White defender replies; with the bug the second
    // White turn tries to replay an occupied cell and solve_puzzle
    // returns false.
    let pack = PuzzlePack::from_path("../../../data/puzzles/vcf-material-763.json");
    let puzzle = pack
        .items
        .iter()
        .find(|p| p.index == 7)
        .expect("vcf-material puzzle 18 should be at pack index 7");

    // The claimed defender replies, in line order.
    let defender_moves: Vec<Move> = puzzle
        .solution
        .iter()
        .filter(|(_, c)| *c != puzzle.attacker)
        .map(|(mv, _)| *mv)
        .collect();
    assert!(
        defender_moves.len() >= 2,
        "regression needs at least two distinct defender replies"
    );
    assert!(
        defender_moves.iter().any(|mv| *mv != defender_moves[0]),
        "defender replies must not all be identical"
    );

    // The deterministic policy, not the real prover, drives the
    // attacker: own probe and guard disabled, one simulation.
    let mut player = AiPlayer::new(Box::new(SolutionFollowingEvaluator::new(puzzle)));
    let options = PlayerOptions {
        simulations: 1,
        c_puct: 1.5,
        ..PlayerOptions::default()
    };
    let mut rng = StdRng::seed_from_u64(1);
    let (_solved, replies) = solve_puzzle_with(puzzle, &mut player, &options, &mut rng);

    // Actual reply advancement: the solver must have used the
    // successive defender replies from the line, in order — not a
    // superficial reply count, and not the first defender move
    // replayed. (Play may continue past the claimed line with random
    // fallback replies, so only the in-line prefix is compared.)
    assert!(
        replies.len() >= 2,
        "solver should reach at least two defender replies, got {replies:?}"
    );
    assert!(
        replies.iter().any(|mv| *mv != replies[0]),
        "solver should use successive distinct defender replies, got {replies:?}"
    );
    for (i, reply) in replies.iter().enumerate().take(defender_moves.len()) {
        assert_eq!(
            *reply, defender_moves[i],
            "defender reply {i} must be the line's {i}-th defender move"
        );
    }
}

// ------------------------------------------------------------------
// Preserved ignored A/B: quiet-whisper measurement (sub-chapter 07)
//
// HISTORICAL evaluator-isolation experiment: raw `mcts::search`, no
// own TSS probe, no guard — matched against the recorded provenance
// (20/20 both arms, 23.5 vs 34.5 average plies, old CLI-probe era).
// Kept probe-free on purpose so the numbers stay comparable with that
// record; it is an isolated-whisper measurement, NOT a full-seat
// strength claim. The production player stack is measured by
// `strength_ladder_measured` below.
// ------------------------------------------------------------------

/// The baseline's move at the given simulation budget, raw search
/// (probe-free, guard-free: the historical experiment's path).
fn raw_baseline_best_move(
    board: &Board,
    simulations: u32,
    rng: &mut StdRng,
    ev: &mut BaselineEvaluator,
) -> Move {
    let cfg = mcts::SearchConfig {
        simulations,
        c_puct: 1.5,
    };
    let outcome = mcts::search(board, ev, &cfg);
    let dist = mcts::visit_distribution(&outcome.tree, outcome.root);
    mcts::select_move(&dist, 0.0, rng).expect("root has legal edges")
}

/// The tactics mock's move at the given simulation budget, raw search.
fn raw_tactics_best_move(
    board: &Board,
    simulations: u32,
    rng: &mut StdRng,
    ev: &mut TacticsEvaluator,
) -> Move {
    let cfg = mcts::SearchConfig {
        simulations,
        c_puct: 1.5,
    };
    let outcome = mcts::search(board, ev, &cfg);
    let dist = mcts::visit_distribution(&outcome.tree, outcome.root);
    mcts::select_move(&dist, 0.0, rng).expect("root has legal edges")
}

/// Run explicitly in release:
/// `cargo test -p baseline --release --test acceptance quiet_whisper_ab_measured -- --ignored --nocapture`
///
/// Protocol: 20 games vs the `TacticsEvaluator` at 200 simulations with
/// alternating colors. One arm uses the default whisper-on evaluator,
/// the other uses `.without_quiet_value()`. Base seed 20_260_901
/// (kept from the sub-chapter 07 record for comparability).
///
/// The test does not assert a win-rate bar: the mock is expected to be
/// too weak to discriminate. The measurement is the result.
#[ignore]
#[test]
fn quiet_whisper_ab_measured() {
    const GAMES: usize = 20;
    const SIMS: u32 = 200;
    const AB_BASE_SEED: u64 = 20_260_901;

    fn play_game_ab(baseline_color: Color, quiet: bool, seed: u64) -> (Status, u32, f64) {
        let mut board = Board::new();
        let mut rng = StdRng::seed_from_u64(seed);
        let mut plies = 0u32;
        let start = Instant::now();
        let mut baseline_ev = if quiet {
            BaselineEvaluator::new(Device::flex())
        } else {
            BaselineEvaluator::new(Device::flex()).without_quiet_value()
        };
        let mut tactics_ev = TacticsEvaluator::new();

        while board.status() == Status::Ongoing && plies < 225 {
            let side = board.to_move();
            let mv = if side == baseline_color {
                raw_baseline_best_move(&board, SIMS, &mut rng, &mut baseline_ev)
            } else {
                raw_tactics_best_move(&board, SIMS, &mut rng, &mut tactics_ev)
            };
            assert!(
                board.is_legal(mv),
                "illegal move {mv:?} at ply {plies} for {side:?}"
            );
            board.play(mv).unwrap();
            plies += 1;
        }
        (board.status(), plies, start.elapsed().as_secs_f64())
    }

    println!("\n=== quiet whisper A/B (historical evaluator-isolation, probe-free) ===\n");
    println!(
        "{:<12} {:>6} {:>6} {:>6} {:>10} {:>12}",
        "arm", "wins", "losses", "draws", "avg_plies", "avg_s/game"
    );
    for quiet in [true, false] {
        let label = if quiet { "whisper ON" } else { "whisper OFF" };
        let mut wins = 0usize;
        let mut losses = 0usize;
        let mut draws = 0usize;
        let mut total_plies = 0u32;
        let mut total_time = 0.0f64;
        for g in 0..GAMES {
            let baseline_color = if g % 2 == 0 {
                Color::Black
            } else {
                Color::White
            };
            let seed = AB_BASE_SEED + g as u64;
            let (status, plies, secs) = play_game_ab(baseline_color, quiet, seed);
            total_plies += plies;
            total_time += secs;
            match status {
                Status::Won(c) if c == baseline_color => wins += 1,
                Status::Won(_) => losses += 1,
                Status::Draw => draws += 1,
                Status::Ongoing => panic!("game {g} did not terminate"),
            }
        }
        println!(
            "{:<12} {:>6} {:>6} {:>6} {:>10.1} {:>12.3}",
            label,
            wins,
            losses,
            draws,
            total_plies as f64 / GAMES as f64,
            total_time / GAMES as f64
        );
    }
}

// ------------------------------------------------------------------
// The measured ladder (ignored; release-only)
// ------------------------------------------------------------------

/// Run explicitly in release:
/// `cargo test -p baseline --release --test acceptance strength_ladder_measured -- --ignored --nocapture`
///
/// Protocol (pre-registered; base seed 20_260_801):
/// 1. Tactical re-check (the two always-on asserts).
/// 2. Sims gradient vs the uniform-random driver: baseline (Black) at
///    400 / 4,000 / 10,000 sims, 10 games per level, seeds
///    `BASE_SEED + g + sims * 10_000`. No bar — a sanity table.
/// 3. Match vs the `TacticsEvaluator`: 20 games at 200 sims both
///    sides, colors alternate by game index, seeds
///    `BASE_SEED + g + 200 * 100_000`. Bar: win rate >= 50%.
/// 4. VCF solve rate: seeded Fisher-Yates prefix sample of 100 puzzles
///    across all six packs; attacker at 4,000 sims through the
///    production `AiPlayer` (own probe fires on the puzzle root);
///    defender follows the pack line indexed by ply (seeded random
///    fallback); solved = attacker wins within `line + 4` plies.
///    Bar: >= 50 solved.
///
/// Metric honesty: `requested_sims` is the sum of configured
/// simulation budgets; `actual_sims` is the sum of root visit totals
/// of decisions that actually searched; a certified own proof credits
/// the full configured budget while running zero simulations, and
/// root TSS work is included in wall time. The reported rate is a
/// configured-budget rate (requested_sims / search_wall_time), NOT
/// leaf-evaluation throughput and not a lower bound — never
/// "evals/sec".
#[ignore]
#[test]
fn strength_ladder_measured() {
    const RANDOM_GAMES: usize = 10;
    const MOCK_GAMES: usize = 20;
    const VCF_SAMPLE: usize = 100;

    let ladder_start = Instant::now();
    println!("\n=== baseline strength ladder (player-level proof path) ===");
    println!("base seed {BASE_SEED}\n");
    flush();

    // 1. Tactical re-check.
    win_in_one_is_solved_at_50_sims();
    forced_block_is_taken_at_50_sims();
    println!("tactical re-check: win-in-1 @50 and forced block @50 hold\n");
    flush();

    // 2. Sims gradient vs uniform-random driver.
    println!("vs uniform-random driver (baseline Black, random White)");
    println!(
        "{:<8} {:>6} {:>6} {:>6} {:>8}",
        "sims", "wins", "losses", "draws", "win_rate"
    );
    flush();
    for sims in [400, 4_000, 10_000] {
        let mut wins = 0;
        let mut losses = 0;
        let mut draws = 0;
        for g in 0..RANDOM_GAMES {
            let seed = BASE_SEED + g as u64 + sims as u64 * 10_000;
            let start = Instant::now();
            let (status, plies, metrics) = play_game(Brain::Baseline(sims), Brain::Random, seed);
            let secs = start.elapsed().as_secs_f64();
            let outcome = match status {
                Status::Won(Color::Black) => {
                    wins += 1;
                    "W"
                }
                Status::Won(Color::White) => {
                    losses += 1;
                    "L"
                }
                Status::Draw => {
                    draws += 1;
                    "D"
                }
                Status::Ongoing => panic!("game did not terminate"),
            };
            println!(
                "  game {g}: {outcome} in {plies} plies, {secs:.1}s, sims requested {} actual {}, proofs {}",
                metrics.requested_sims, metrics.actual_sims, metrics.proofs
            );
            flush();
        }
        let rate = wins as f64 / RANDOM_GAMES as f64;
        println!(
            "{:<8} {:>6} {:>6} {:>6} {:>7.0}%",
            sims,
            wins,
            losses,
            draws,
            rate * 100.0
        );
        flush();
    }

    // 3. Match vs TacticsEvaluator at 200 sims; colors alternate by
    //    game index so first-move advantage stays out of the score.
    println!("\nvs TacticsEvaluator at 200 sims");
    println!(
        "{:<8} {:>6} {:>6} {:>6} {:>8}",
        "games", "wins", "losses", "draws", "win_rate"
    );
    flush();
    {
        let mut wins = 0;
        let mut losses = 0;
        let mut draws = 0;
        let mut total_requested_sims = 0u64;
        let mut total_actual_sims = 0u64;
        let mut total_proofs = 0u64;
        let mut total_search_time = 0.0f64;
        let mut total_game_time = 0.0f64;
        for g in 0..MOCK_GAMES {
            let seed = BASE_SEED + g as u64 + 200 * 100_000;
            let (black, white) = if g % 2 == 0 {
                (Brain::Baseline(200), Brain::Tactics(200))
            } else {
                (Brain::Tactics(200), Brain::Baseline(200))
            };
            let start = Instant::now();
            let (status, plies, metrics) = play_game(black, white, seed);
            let game_secs = start.elapsed().as_secs_f64();
            total_game_time += game_secs;
            total_requested_sims += metrics.requested_sims;
            total_actual_sims += metrics.actual_sims;
            total_proofs += metrics.proofs;
            total_search_time += metrics.search_time_secs;

            let baseline_won = match status {
                Status::Won(Color::Black) => g % 2 == 0,
                Status::Won(Color::White) => g % 2 == 1,
                Status::Draw => {
                    draws += 1;
                    println!("  game {g}: D in {plies} plies, {game_secs:.1}s");
                    flush();
                    continue;
                }
                Status::Ongoing => panic!("game {g} did not terminate"),
            };
            if baseline_won {
                wins += 1;
            } else {
                losses += 1;
            }
            println!(
                "  game {g}: {} in {plies} plies, {game_secs:.1}s, sims requested {} actual {}, proofs {}",
                if baseline_won { "W" } else { "L" },
                metrics.requested_sims,
                metrics.actual_sims,
                metrics.proofs
            );
            flush();
        }
        let rate = wins as f64 / MOCK_GAMES as f64;
        println!(
            "{:<8} {:>6} {:>6} {:>6} {:>7.0}%",
            MOCK_GAMES,
            wins,
            losses,
            draws,
            rate * 100.0
        );
        println!(
            "  seconds/game: {:.3}, configured_budget_rate: {:.0} requested_sims/search_wall_time (not leaf-eval throughput; a root proof may credit the full budget with zero simulations; root overhead included in wall time)",
            total_game_time / MOCK_GAMES as f64,
            total_requested_sims as f64 / total_search_time
        );
        println!(
            "  totals: requested_sims {total_requested_sims}, actual_sims {total_actual_sims}, proofs {total_proofs}"
        );
        flush();
        assert!(
            rate >= 0.50,
            "baseline must win at least 50% against TacticsEvaluator at 200 sims"
        );
    }

    // 4. VCF solve rate across all six packs (kaibao + four renju
    //    shards are engine-proven conversions; vcf-material-763 is
    //    source-claimed pending milestone-4 re-verification — the bar
    //    below includes it, record what you get).
    println!("\nVCF solve rate");
    flush();
    {
        let named_packs = [
            (
                "kaibao",
                PuzzlePack::from_path("../../../data/puzzles/kaibao-vcf-676.json"),
            ),
            (
                "renju-1",
                PuzzlePack::from_path("../../../data/puzzles/renju-portal-vcf-783-1of4.json"),
            ),
            (
                "renju-2",
                PuzzlePack::from_path("../../../data/puzzles/renju-portal-vcf-783-2of4.json"),
            ),
            (
                "renju-3",
                PuzzlePack::from_path("../../../data/puzzles/renju-portal-vcf-783-3of4.json"),
            ),
            (
                "renju-4",
                PuzzlePack::from_path("../../../data/puzzles/renju-portal-vcf-783-4of4.json"),
            ),
            (
                "material",
                PuzzlePack::from_path("../../../data/puzzles/vcf-material-763.json"),
            ),
        ];

        // Seeded sample of VCF_SAMPLE puzzles across all packs.
        let mut all: Vec<(usize, usize)> = Vec::new();
        for (pack_idx, (_, pack)) in named_packs.iter().enumerate() {
            for idx in 0..pack.items.len() {
                all.push((pack_idx, idx));
            }
        }
        let total_puzzles = all.len();

        let mut rng = StdRng::seed_from_u64(BASE_SEED);
        // Fisher-Yates shuffle a prefix sample.
        for i in 0..all.len().min(VCF_SAMPLE) {
            let j = rng.random_range(i..all.len());
            all.swap(i, j);
        }
        all.truncate(VCF_SAMPLE);

        let mut solved = 0;
        let vcf_start = Instant::now();
        for (idx, (pack_idx, puzzle_idx)) in all.iter().enumerate() {
            let puzzle = &named_packs[*pack_idx].1.items[*puzzle_idx];
            let seed = BASE_SEED + idx as u64;
            let start = Instant::now();
            let ok = solve_puzzle(puzzle, 4_000, seed);
            if ok {
                solved += 1;
            }
            println!(
                "  puzzle {idx} ({} #{puzzle_idx}): {} in {:.1}s",
                named_packs[*pack_idx].0,
                if ok { "solved" } else { "UNSOLVED" },
                start.elapsed().as_secs_f64()
            );
            if (idx + 1) % 10 == 0 {
                println!(
                    "  VCF progress: {}/{} solved in {:.1}s",
                    solved,
                    idx + 1,
                    vcf_start.elapsed().as_secs_f64()
                );
            }
            flush();
        }
        println!("solved {solved}/{VCF_SAMPLE} (sampled from {total_puzzles})");
        flush();
        assert!(
            solved >= VCF_SAMPLE / 2,
            "baseline must solve at least half of the {VCF_SAMPLE} VCF puzzles"
        );
    }

    println!(
        "\nladder wall time: {:.1} min",
        ladder_start.elapsed().as_secs_f64() / 60.0
    );
    flush();
}

// ------------------------------------------------------------------
// Interactive latency probe (ignored; release-only)
// ------------------------------------------------------------------

/// Run explicitly in release:
/// `cargo test -p baseline --release --test acceptance interactive_latency_measured -- --ignored --nocapture`
///
/// One seeded legal game: the production baseline seat (own probe +
/// guard, exactly the CLI policy) at 400 simulations — the
/// interactive default — against the tactics seat at 400. Every
/// baseline decision is timed and printed; the summary reports
/// p50/p95/max, the proof count, and requested vs actual sims.
///
/// Evaluator/player construction is warmup and is excluded from every
/// statistic. There is deliberately NO latency bar and no speed
/// guarantee here: this records what the seat costs, and any
/// interactive-budget recommendation is chosen after measuring, not
/// before.
#[ignore]
#[test]
fn interactive_latency_measured() {
    const SIMS: u32 = 400;

    println!("\n=== interactive latency: baseline at {SIMS} sims vs tactics at {SIMS} ===");
    // Warmup: evaluator/player construction (ThreatNet load) is
    // excluded from all decision statistics.
    let construct_start = Instant::now();
    let mut baseline = AiPlayer::new(Box::new(BaselineEvaluator::new(Device::flex())));
    let mut tactics = AiPlayer::new(Box::new(TacticsEvaluator::new()));
    println!(
        "construction (warmup, excluded): {:.1} ms",
        construct_start.elapsed().as_secs_f64() * 1000.0
    );
    flush();

    let base_options = baseline_options(SIMS);
    let tactics_options = tactics_options(SIMS);
    let mut rng = StdRng::seed_from_u64(BASE_SEED + 9_000_001);
    let mut board = Board::new();
    let mut plies = 0u32;
    let mut latencies_ms: Vec<f64> = Vec::new();
    let mut proofs = 0u64;
    let mut requested_total = 0u64;
    let mut actual_total = 0u64;

    while board.status() == Status::Ongoing && plies < 225 {
        let side = board.to_move();
        let mv = if side == Color::Black {
            let start = Instant::now();
            let decision = baseline
                .decide(&board, &base_options, &mut rng)
                .expect("game ongoing");
            let ms = start.elapsed().as_secs_f64() * 1000.0;
            latencies_ms.push(ms);
            requested_total += u64::from(SIMS);
            match &decision.search {
                Some(stats) => {
                    let actual: u64 = stats.visits.iter().map(|(_, n)| u64::from(*n)).sum();
                    actual_total += actual;
                    println!(
                        "  ply {plies}: {ms:.1} ms (searched, {actual} root visits of {SIMS} requested)"
                    );
                }
                None => {
                    proofs += 1;
                    println!(
                        "  ply {plies}: {ms:.1} ms (certified own proof, 0 sims of {SIMS} requested)"
                    );
                }
            }
            flush();
            decision.mv
        } else {
            best_move(&mut tactics, &board, &tactics_options, &mut rng)
        };
        assert!(
            board.is_legal(mv),
            "illegal move {mv:?} at ply {plies} for {side:?}"
        );
        board.play(mv).unwrap();
        plies += 1;
    }

    assert!(
        board.status() != Status::Ongoing || plies == 225,
        "the latency game must terminate"
    );
    latencies_ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let percentile = |p: usize| -> f64 {
        let idx = (latencies_ms.len() * p / 100).min(latencies_ms.len() - 1);
        latencies_ms[idx]
    };
    println!("\nstatus {:?} after {plies} plies", board.status());
    println!(
        "decisions {}, proofs {proofs}, sims requested {requested_total} actual {actual_total}",
        latencies_ms.len()
    );
    if !latencies_ms.is_empty() {
        println!(
            "latency ms: p50 {:.1}, p95 {:.1}, max {:.1}",
            percentile(50),
            percentile(95),
            latencies_ms[latencies_ms.len() - 1]
        );
    }
    flush();
}
