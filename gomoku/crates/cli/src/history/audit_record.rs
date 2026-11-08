//! CLI-owned v3 audit DTOs. Moves enter only through checked coordinates.
use super::record::checked_square;
use super::{Resignation, ResignationAudit, ResignationEvent};
use crate::actor::{AiConfig, EvaluatorKind, SearchInfo};
use engine::{Board, Color, Move, Proof, SearchBudget, Status};
use player::{GuardConfig, OwnTssConfig, PlayerOptions, RefutedCandidate, ResignationEvidence};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ReasonRecord {
    VerifiedOpponentVcf,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EventRecord {
    reason: ReasonRecord,
    by: Color,
    candidates: usize,
    resumed: bool,
    evidence: EvidenceRecord,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum EvidenceRecord {
    Original {
        options: OptionsRecord,
        search: SearchRecord,
        candidates: Vec<CandidateRecord>,
    },
    LegacyUnavailable {},
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BudgetRecord {
    max_nodes: u32,
    max_depth: u8,
}

impl From<SearchBudget> for BudgetRecord {
    fn from(budget: SearchBudget) -> Self {
        Self {
            max_nodes: budget.max_nodes,
            max_depth: budget.max_depth,
        }
    }
}
impl From<&BudgetRecord> for SearchBudget {
    fn from(budget: &BudgetRecord) -> Self {
        Self {
            max_nodes: budget.max_nodes,
            max_depth: budget.max_depth,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnRecord {
    enabled: bool,
    budget: BudgetRecord,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GuardRecord {
    enabled: bool,
    max_candidates: usize,
    budget: BudgetRecord,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OptionsRecord {
    simulations: u32,
    c_puct: f32,
    temperature: f32,
    noise_epsilon: f32,
    noise_alpha: f32,
    own: OwnRecord,
    guard: GuardRecord,
    resign: bool,
}

impl OptionsRecord {
    fn capture(options: PlayerOptions) -> Self {
        Self {
            simulations: options.simulations,
            c_puct: options.c_puct,
            temperature: options.temperature,
            noise_epsilon: options.noise_epsilon,
            noise_alpha: options.noise_alpha,
            own: OwnRecord {
                enabled: options.own.enabled,
                budget: options.own.budget.into(),
            },
            guard: GuardRecord {
                enabled: options.guard.enabled,
                max_candidates: options.guard.max_candidates,
                budget: options.guard.budget.into(),
            },
            resign: options.resign,
        }
    }

    fn restore(&self, config: &AiConfig) -> Result<PlayerOptions, String> {
        // Historical evaluator identities do not require a compiled live evaluator.
        crate::session::record::validate_config_values(config)?;
        let own = OwnTssConfig {
            enabled: self.own.enabled,
            budget: (&self.own.budget).into(),
        };
        let guard = GuardConfig {
            enabled: self.guard.enabled,
            max_candidates: self.guard.max_candidates,
            budget: (&self.guard.budget).into(),
        };
        let (expected_own, expected_guard) = match config.evaluator {
            EvaluatorKind::Tactics => (OwnTssConfig::disabled(), GuardConfig::disabled()),
            EvaluatorKind::Baseline => (OwnTssConfig::baseline(), GuardConfig::baseline()),
        };
        if !config.resign
            || !self.resign
            || self.simulations != config.simulations
            || self.c_puct != config.c_puct
            || self.temperature != config.temperature
            || self.noise_epsilon != config.noise_epsilon
            || self.noise_alpha != 0.3
            || own != expected_own
            || guard != expected_guard
        {
            return Err("audit options disagree with effective configuration/presets".into());
        }
        // Temperature/noise already describe the effective decision, including
        // automatic opening scheduling; explicit_selection need not be true.
        Ok(PlayerOptions {
            simulations: self.simulations,
            c_puct: self.c_puct,
            temperature: self.temperature,
            noise_epsilon: self.noise_epsilon,
            noise_alpha: self.noise_alpha,
            own,
            guard,
            resign: self.resign,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EdgeRecord {
    square: [u8; 2],
    visits: u32,
    prior: f32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchRecord {
    side_to_move: Color,
    value: f32,
    config: AiConfig,
    edges: Vec<EdgeRecord>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProofRecord {
    winner: Color,
    line: Vec<[u8; 2]>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CandidateRecord {
    ordinal: u8,
    square: [u8; 2],
    visits: u32,
    prior: f32,
    probe_budget: BudgetRecord,
    proof: ProofRecord,
}

fn square(mv: Move) -> [u8; 2] {
    [mv.row(), mv.col()]
}

impl EventRecord {
    pub(super) fn capture(event: &ResignationEvent) -> Self {
        let evidence = match &event.audit {
            None => EvidenceRecord::LegacyUnavailable {},
            Some(audit) => {
                let info = &audit.search;
                let mut edges: Vec<_> = info
                    .visits
                    .iter()
                    .map(|&(mv, visits)| {
                        let prior = info
                            .prior
                            .iter()
                            .find(|&&(m, _)| m == mv)
                            .expect("original root statistics cover the same edges")
                            .1;
                        EdgeRecord {
                            square: square(mv),
                            visits,
                            prior,
                        }
                    })
                    .collect();
                edges.sort_by_key(|edge| edge.square);
                EvidenceRecord::Original {
                    options: OptionsRecord::capture(audit.evidence.options),
                    search: SearchRecord {
                        side_to_move: info.side_to_move,
                        value: info.value,
                        config: info.config.clone(),
                        edges,
                    },
                    candidates: audit
                        .evidence
                        .candidates
                        .iter()
                        .map(|candidate| CandidateRecord {
                            ordinal: candidate.ordinal,
                            square: square(candidate.mv),
                            visits: candidate.visits,
                            prior: candidate.prior,
                            probe_budget: candidate.probe_budget.into(),
                            proof: ProofRecord {
                                winner: candidate.proof.winner,
                                line: candidate.proof.line.iter().copied().map(square).collect(),
                            },
                        })
                        .collect(),
                }
            }
        };
        Self {
            reason: ReasonRecord::VerifiedOpponentVcf,
            by: event.summary.by,
            candidates: event.summary.candidates,
            resumed: event.resumed,
            evidence,
        }
    }

    /// Check cheap shapes for the whole file before any proof verification.
    pub(super) fn validate_shape(&self, is_latest: bool) -> Result<(), String> {
        if self.candidates == 0 || self.candidates > 5 || (!is_latest && !self.resumed) {
            return Err("invalid concession count or resume ordering".into());
        }
        if let EvidenceRecord::Original {
            search, candidates, ..
        } = &self.evidence
        {
            if candidates.len() != self.candidates || search.edges.len() > 225 {
                return Err("invalid audit candidate/edge count".into());
            }
            for edge in &search.edges {
                checked_square(edge.square)?;
            }
            for candidate in candidates {
                checked_square(candidate.square)?;
                let budget: SearchBudget = (&candidate.probe_budget).into();
                let len = candidate.proof.line.len();
                if budget != GuardConfig::baseline().budget
                    || len == 0
                    || len.is_multiple_of(2)
                    || len > usize::from(budget.max_depth)
                {
                    return Err("invalid original proof budget/line shape".into());
                }
                for &mv in &candidate.proof.line {
                    checked_square(mv)?;
                }
            }
        }
        Ok(())
    }

    /// Restore and validate all non-proof invariants; verification is a later pass.
    pub(super) fn restore(&self, board: &Board) -> Result<ResignationEvent, String> {
        let legal_count = board.empty_moves().count();
        if board.status() != Status::Ongoing
            || self.by != board.to_move()
            || self.candidates == 0
            || self.candidates > legal_count.min(5)
        {
            return Err("invalid resignation annotation".into());
        }
        let audit = match &self.evidence {
            EvidenceRecord::LegacyUnavailable {} => None,
            EvidenceRecord::Original {
                options,
                search,
                candidates,
            } => {
                if self.candidates != legal_count.min(5)
                    || search.edges.len() != legal_count
                    || search.side_to_move != self.by
                    || !search.value.is_finite()
                    || !(-1.0..=1.0).contains(&search.value)
                    || !engine::immediate_wins(board, self.by).is_empty()
                {
                    return Err("invalid original audit root/count/value".into());
                }
                let options = options.restore(&search.config)?;
                let mut seen = [false; 225];
                let mut ranked = Vec::with_capacity(legal_count);
                let mut total_visits = 0u64;
                let mut total_prior = 0.0f64;
                for edge in &search.edges {
                    let mv = checked_square(edge.square)?;
                    if !board.is_legal(mv)
                        || std::mem::replace(&mut seen[mv.index()], true)
                        || !edge.prior.is_finite()
                        || !(0.0..=1.0).contains(&edge.prior)
                    {
                        return Err("illegal/duplicate edge or invalid prior".into());
                    }
                    total_visits += u64::from(edge.visits);
                    total_prior += f64::from(edge.prior);
                    ranked.push((mv, edge.visits, edge.prior));
                }
                if total_visits != u64::from(options.simulations)
                    || (total_prior - 1.0).abs() > 1e-4
                {
                    return Err("root visits/prior totals disagree".into());
                }
                ranked.sort_by(|a, b| {
                    b.1.cmp(&a.1)
                        .then_with(|| b.2.partial_cmp(&a.2).expect("validated finite priors"))
                        .then_with(|| a.0.index().cmp(&b.0.index()))
                });
                let mut restored = Vec::with_capacity(candidates.len());
                for (index, candidate) in candidates.iter().enumerate() {
                    let mv = checked_square(candidate.square)?;
                    if candidate.ordinal as usize != index + 1
                        || (mv, candidate.visits, candidate.prior) != ranked[index]
                    {
                        return Err("candidate ordinal/rank/statistics disagree".into());
                    }
                    let mut successor = board.clone();
                    successor.play(mv).map_err(|e| e.to_string())?;
                    if successor.status() != Status::Ongoing
                        || candidate.proof.winner != self.by.other()
                        || candidate.proof.line.len() > successor.empty_moves().count()
                    {
                        return Err("invalid refutation successor/winner/length".into());
                    }
                    restored.push(RefutedCandidate {
                        ordinal: candidate.ordinal,
                        mv,
                        visits: candidate.visits,
                        prior: candidate.prior,
                        probe_budget: (&candidate.probe_budget).into(),
                        proof: Proof {
                            winner: candidate.proof.winner,
                            line: candidate
                                .proof
                                .line
                                .iter()
                                .copied()
                                .map(checked_square)
                                .collect::<Result<_, _>>()?,
                        },
                    });
                }
                let mut visits: Vec<_> = ranked.iter().map(|&(mv, n, _)| (mv, n)).collect();
                visits.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.index().cmp(&b.0.index())));
                let mut prior: Vec<_> = ranked.iter().map(|&(mv, _, p)| (mv, p)).collect();
                prior.sort_by(|a, b| {
                    b.1.partial_cmp(&a.1)
                        .expect("validated finite priors")
                        .then_with(|| a.0.index().cmp(&b.0.index()))
                });
                Some(ResignationAudit {
                    evidence: ResignationEvidence {
                        options,
                        candidates: restored,
                    },
                    search: SearchInfo {
                        side_to_move: search.side_to_move,
                        visits,
                        prior,
                        value: search.value,
                        config: search.config.clone(),
                    },
                })
            }
        };
        Ok(ResignationEvent {
            summary: Resignation {
                by: self.by,
                candidates: self.candidates,
            },
            resumed: self.resumed,
            audit,
        })
    }
}

pub(super) fn verify_original(board: &Board, event: &ResignationEvent) -> Result<(), String> {
    if let Some(audit) = &event.audit {
        for candidate in &audit.evidence.candidates {
            let mut successor = board.clone();
            successor.play(candidate.mv).map_err(|e| e.to_string())?;
            if !engine::verify_line(&successor, &candidate.proof) {
                return Err("invalid original candidate proof".into());
            }
        }
    }
    Ok(())
}
