use crate::euclid::ast::{Output, VolumeSplit};
use crate::euclid::{ast, types};
use rand::distributions::WeightedIndex;
use rand::prelude::*;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;

use super::ast::ConnectorInfo;

pub struct InterpreterBackend {
    _program: ast::Program,
}

impl InterpreterBackend {
    fn eval_number_comparison_array(
        num: u64,
        array: &[ast::NumberComparison],
    ) -> Result<bool, types::InterpreterError> {
        for comparison in array {
            let other = comparison.number;
            let passed = match comparison.comparison_type {
                ast::ComparisonType::GreaterThan => num > other,
                ast::ComparisonType::LessThan => num < other,
                ast::ComparisonType::LessThanEqual => num <= other,
                ast::ComparisonType::GreaterThanEqual => num >= other,
                ast::ComparisonType::Equal => num == other,
                ast::ComparisonType::NotEqual => num != other,
            };
            if !passed {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn eval_comparison(
        comparison: &ast::Comparison,
        ctx: &types::Context,
        globals: &ast::Globals,
    ) -> Result<bool, types::InterpreterError> {
        use ast::{ComparisonType::*, ValueType::*};

        let (lookup_key, ctx_value) = match &comparison.value {
            MetadataVariant(metadata) => (
                &metadata.key,
                ctx.get(&metadata.key)
                    .filter(|value| value.as_ref().is_some_and(ast::ValueType::is_metadata))
                    .or_else(|| ctx.get(&comparison.lhs)),
            ),
            _ => (&comparison.lhs, ctx.get(&comparison.lhs)),
        };

        if ctx_value.is_none() {
            crate::logger::debug!(
                missing_context_key = %lookup_key,
                condition_lhs = %comparison.lhs,
                "Context key not found while evaluating condition, skipping rule"
            );
            return Ok(false);
        }

        let value = ctx_value.and_then(|v| v.as_ref());

        if let Some(val) = value {
            match (val, &comparison.comparison, &comparison.value) {
                (EnumVariant(e1), Equal, EnumVariant(e2)) => Ok(e1 == e2),
                (EnumVariant(e1), NotEqual, EnumVariant(e2)) => Ok(e1 != e2),
                (EnumVariant(e), Equal, EnumVariantArray(evec)) => Ok(evec.iter().any(|v| e == v)),
                (EnumVariant(e), NotEqual, EnumVariantArray(evec)) => {
                    Ok(evec.iter().all(|v| e != v))
                }
                (Number(n1), Equal, Number(n2)) => Ok(n1 == n2),
                (Number(n1), NotEqual, Number(n2)) => Ok(n1 != n2),
                (Number(n1), LessThanEqual, Number(n2)) => Ok(n1 <= n2),
                (Number(n1), GreaterThanEqual, Number(n2)) => Ok(n1 >= n2),
                (Number(n1), LessThan, Number(n2)) => Ok(n1 < n2),
                (Number(n1), GreaterThan, Number(n2)) => Ok(n1 > n2),
                (Number(n), Equal, NumberArray(nvec)) => Ok(nvec.iter().any(|v| v == n)),
                (Number(n), NotEqual, NumberArray(nvec)) => Ok(nvec.iter().all(|v| v != n)),
                (Number(n), Equal, NumberComparisonArray(ncvec)) => {
                    Self::eval_number_comparison_array(*n, ncvec)
                }
                (MetadataVariant(m1), Equal, MetadataVariant(m2)) => Ok(m1 == m2),
                (MetadataVariant(m1), NotEqual, MetadataVariant(m2)) => Ok(m1 != m2),
                (StrValue(s1), Equal, StrValue(s2)) => Ok(s1 == s2),
                (StrValue(s1), NotEqual, StrValue(s2)) => Ok(s1 != s2),
                (val, Equal, GlobalRef(name)) => Ok(globals
                    .get(name)
                    .map(|set| set.contains(val))
                    .unwrap_or(false)),
                _ => Err(types::InterpreterError {
                    error_type: types::InterpreterErrorType::InvalidComparison,
                    metadata: comparison.metadata.clone(),
                }),
            }
        } else {
            Ok(false)
        }
    }

    fn eval_if_condition(
        condition: &ast::IfCondition,
        ctx: &types::Context,
        globals: &ast::Globals,
    ) -> Result<bool, types::InterpreterError> {
        for comparison in condition {
            let res = Self::eval_comparison(comparison, ctx, globals)?;

            if !res {
                return Ok(false);
            }
        }

        Ok(true)
    }

    fn eval_if_statement(
        stmt: &ast::IfStatement,
        ctx: &types::Context,
        globals: &ast::Globals,
    ) -> Result<bool, types::InterpreterError> {
        let cond_res = Self::eval_if_condition(&stmt.condition, ctx, globals)?;

        if !cond_res {
            return Ok(false);
        }

        if let Some(ref nested) = stmt.nested {
            for nested_if in nested {
                let res = Self::eval_if_statement(nested_if, ctx, globals)?;

                if res {
                    return Ok(true);
                }
            }

            return Ok(false);
        }

        Ok(true)
    }

    fn eval_rule_statements(
        statements: &[ast::IfStatement],
        ctx: &types::Context,
        globals: &ast::Globals,
    ) -> Result<bool, types::InterpreterError> {
        for stmt in statements {
            let res = Self::eval_if_statement(stmt, ctx, globals)?;

            if res {
                return Ok(true);
            }
        }

        Ok(false)
    }

    #[inline]
    fn eval_rule(
        rule: &ast::Rule,
        ctx: &types::Context,
        globals: &ast::Globals,
    ) -> Result<bool, types::InterpreterError> {
        Self::eval_rule_statements(&rule.statements, ctx, globals)
    }

    pub fn eval_program(
        program: &ast::Program,
        ctx: &types::Context,
        seed: Option<&str>,
    ) -> Result<types::BackendOutput, types::InterpreterError> {
        for rule in &program.rules {
            let res = Self::eval_rule(rule, ctx, &program.globals)?;

            if res {
                let evaluated_output =
                    evaluate_output(&rule.output, seed).map_err(|e| types::InterpreterError {
                        error_type: types::InterpreterErrorType::OutputEvaluationFailed(format!(
                            "{:?}",
                            e
                        )),
                        metadata: HashMap::new(),
                    })?;
                return Ok(types::BackendOutput {
                    rule_name: Some(rule.name.clone()),
                    output: rule.output.clone(),
                    evaluated_output,
                });
            }
        }

        // If no rule matched, evaluate default selection
        let evaluated_output = evaluate_output(&program.default_selection, seed).map_err(|e| {
            types::InterpreterError {
                error_type: types::InterpreterErrorType::OutputEvaluationFailed(format!("{:?}", e)),
                metadata: HashMap::new(),
            }
        })?;

        Ok(types::BackendOutput {
            rule_name: None,
            output: program.default_selection.clone(),
            evaluated_output,
        })
    }
}

#[derive(Debug)]
pub enum RoutingError {
    VolumeSplitFailed,
}

impl fmt::Display for RoutingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VolumeSplitFailed => write!(f, "Volume split calculation failed"),
        }
    }
}
impl Error for RoutingError {}
type RoutingResult<T> = Result<T, RoutingError>;

/// djb2 hash of the volume-split seed. Byte-identical with the A/B arm assignment
/// (`ab_test::common::assign_arm` / `ab_test::evaluator`) and with Hyperswitch's
/// seeded volume split (hyperswitch `crates/router/src/core/payments/routing.rs`,
/// `seeded_volume_split_index`): both sides must land on the same winner for the
/// same payment, so this hash is a cross-repo contract — do not change it alone.
fn djb2_seed_hash(seed: &str) -> u64 {
    seed.bytes().fold(5381u64, |acc, b| {
        acc.wrapping_mul(33).wrapping_add(u64::from(b))
    })
}

fn sample_split_winner_first<T>(
    mut splits: Vec<VolumeSplit<T>>,
    seed: Option<&str>,
) -> RoutingResult<Vec<T>> {
    let idx = match seed {
        // Deterministic per seed: djb2 slot over the cumulative weights, walked in
        // declaration order. Every evaluation of the same payment picks the same
        // winner, so the SDK's concurrent PML/session/config calls — and retries —
        // cannot disagree with each other or with Hyperswitch's local evaluation.
        Some(seed) => {
            let total_weight: u64 = splits.iter().map(|sp| u64::from(sp.split)).sum();
            if total_weight == 0 {
                return Err(RoutingError::VolumeSplitFailed);
            }
            let slot = djb2_seed_hash(seed) % total_weight;
            let mut cumulative = 0u64;
            splits
                .iter()
                .position(|split| {
                    cumulative += u64::from(split.split);
                    slot < cumulative
                })
                .ok_or(RoutingError::VolumeSplitFailed)?
        }
        None => {
            let weights: Vec<u8> = splits.iter().map(|sp| sp.split).collect();
            let weighted_index =
                WeightedIndex::new(weights).map_err(|_| RoutingError::VolumeSplitFailed)?;
            let mut rng = rand::thread_rng();
            weighted_index.sample(&mut rng)
        }
    };

    if idx >= splits.len() {
        return Err(RoutingError::VolumeSplitFailed);
    }
    let winner = splits.remove(idx);
    splits.insert(0, winner);

    Ok(splits.into_iter().map(|split| split.output).collect())
}

pub fn perform_volume_split(
    splits: Vec<VolumeSplit<ConnectorInfo>>,
    seed: Option<&str>,
) -> RoutingResult<Vec<ConnectorInfo>> {
    sample_split_winner_first(splits, seed)
}

pub fn perform_volume_split_priority(
    splits: Vec<VolumeSplit<Vec<ConnectorInfo>>>,
    seed: Option<&str>,
) -> RoutingResult<Vec<ConnectorInfo>> {
    Ok(sample_split_winner_first(splits, seed)?
        .into_iter()
        .flatten()
        .fold(Vec::new(), |mut ordered, connector| {
            if !ordered.contains(&connector) {
                ordered.push(connector);
            }
            ordered
        }))
}

pub fn evaluate_output(output: &Output, seed: Option<&str>) -> RoutingResult<Vec<ConnectorInfo>> {
    match output {
        Output::Single(connector) => Ok(vec![connector.clone()]),
        Output::Priority(connectors) => Ok(connectors.clone()),
        Output::VolumeSplit(splits) => perform_volume_split(splits.clone(), seed),
        Output::VolumeSplitPriority(splits) => perform_volume_split_priority(splits.clone(), seed),
    }
}

#[cfg(test)]
mod seeded_volume_split_tests {
    use super::*;

    fn splits(weights: &[u8]) -> Vec<VolumeSplit<ConnectorInfo>> {
        weights
            .iter()
            .enumerate()
            .map(|(i, w)| VolumeSplit {
                split: *w,
                output: ConnectorInfo {
                    gateway_name: format!("gw_{i}"),
                    gateway_id: None,
                },
            })
            .collect()
    }

    // Cross-repo contract: Hyperswitch's seeded_volume_split_index test asserts the
    // same vector. If this changes, the two engines stop agreeing per payment.
    #[test]
    fn djb2_matches_the_cross_repo_vector() {
        assert_eq!(
            djb2_seed_hash("pay_nZwbogscgFwIanlGnUxw"),
            10501740297535541692
        );
        assert_eq!(djb2_seed_hash(""), 5381);
    }

    #[test]
    fn same_seed_always_picks_the_same_winner() {
        let first = perform_volume_split(splits(&[50, 50]), Some("pay_abc123")).unwrap();
        for _ in 0..100 {
            let again = perform_volume_split(splits(&[50, 50]), Some("pay_abc123")).unwrap();
            assert_eq!(again, first);
        }
    }

    #[test]
    fn winner_moves_to_front_and_rest_keep_declaration_order() {
        // djb2("pay_z") % 100 selects a definite slot; whatever wins, the remaining
        // entries must keep their original relative order.
        let out = perform_volume_split(splits(&[10, 20, 30, 40]), Some("pay_z")).unwrap();
        let rest: Vec<_> = out[1..].iter().map(|c| c.gateway_name.clone()).collect();
        let mut expected: Vec<_> = (0..4)
            .map(|i| format!("gw_{i}"))
            .filter(|name| *name != out[0].gateway_name)
            .collect();
        expected.sort_by_key(|name| name.clone());
        let mut rest_sorted = rest.clone();
        rest_sorted.sort();
        assert_eq!(rest_sorted, expected);
        assert_eq!(rest, {
            let mut in_order: Vec<_> = (0..4).map(|i| format!("gw_{i}")).collect();
            in_order.retain(|name| *name != out[0].gateway_name);
            in_order
        });
    }

    #[test]
    fn seeded_split_respects_weights_across_many_payments() {
        // 80/20 split over 10k distinct payment ids should land near 80/20 —
        // the hash spreads payments uniformly over the weight range.
        let mut first_wins = 0u32;
        for i in 0..10_000 {
            let seed = format!("pay_{i}");
            let out = perform_volume_split(splits(&[80, 20]), Some(&seed)).unwrap();
            if out[0].gateway_name == "gw_0" {
                first_wins += 1;
            }
        }
        assert!((7_500..=8_500).contains(&first_wins), "got {first_wins}");
    }

    #[test]
    fn zero_total_weight_errors_instead_of_dividing_by_zero() {
        assert!(perform_volume_split(splits(&[0, 0]), Some("pay_x")).is_err());
    }

    #[test]
    fn unseeded_path_still_works() {
        let out = perform_volume_split(splits(&[50, 50]), None).unwrap();
        assert_eq!(out.len(), 2);
    }
}
