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
    fn eval_comparison(
        comparison: &ast::Comparison,
        ctx: &types::Context,
        globals: &ast::Globals,
    ) -> Result<bool, types::InterpreterError> {
        use ast::{ComparisonType::*, ValueType::*};

        let ctx_value = ctx.get(&comparison.lhs);
        if ctx_value.is_none() {
            crate::logger::warn!(
                missing_context_key = %comparison.lhs,
                "Context key not found while evaluating condition, skipping rule"
            );
            return Ok(false);
        }

        let value = ctx_value.and_then(|v| v.as_ref());

        if let Some(val) = value {
            match (val, &comparison.comparison, &comparison.value) {
                (EnumVariant(e1), Equal, EnumVariant(e2)) => Ok(e1 == e2),
                (EnumVariant(e1), NotEqual, EnumVariant(e2)) => Ok(e1 != e2),
                (Number(n1), Equal, Number(n2)) => Ok(n1 == n2),
                (Number(n1), NotEqual, Number(n2)) => Ok(n1 != n2),
                (Number(n1), LessThanEqual, Number(n2)) => Ok(n1 <= n2),
                (Number(n1), GreaterThanEqual, Number(n2)) => Ok(n1 >= n2),
                (Number(n1), LessThan, Number(n2)) => Ok(n1 < n2),
                (Number(n1), GreaterThan, Number(n2)) => Ok(n1 > n2),
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
    ) -> Result<types::BackendOutput, types::InterpreterError> {
        for rule in &program.rules {
            let res = Self::eval_rule(rule, ctx, &program.globals)?;

            if res {
                let (_, evaluated_output) =
                    evaluate_output(&rule.output).map_err(|e| types::InterpreterError {
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
        let (_, evaluated_output) =
            evaluate_output(&program.default_selection).map_err(|e| types::InterpreterError {
                error_type: types::InterpreterErrorType::OutputEvaluationFailed(format!("{:?}", e)),
                metadata: HashMap::new(),
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
            RoutingError::VolumeSplitFailed => write!(f, "Volume split calculation failed"),
        }
    }
}
impl Error for RoutingError {}
type RoutingResult<T> = Result<T, RoutingError>;

pub fn perform_volume_split(
    splits: Vec<VolumeSplit<ConnectorInfo>>,
) -> RoutingResult<ConnectorInfo> {
    let weights: Vec<u8> = splits.iter().map(|sp| sp.split).collect();
    let weighted_index =
        WeightedIndex::new(weights).map_err(|_| RoutingError::VolumeSplitFailed)?;
    let mut rng = rand::thread_rng();
    let idx = weighted_index.sample(&mut rng);
    splits
        .get(idx)
        .map(|split| split.output.clone())
        .ok_or(RoutingError::VolumeSplitFailed)
}

pub fn perform_volume_split_priority(
    splits: Vec<VolumeSplit<Vec<ConnectorInfo>>>,
) -> RoutingResult<Vec<ConnectorInfo>> {
    let weights: Vec<u8> = splits.iter().map(|sp| sp.split).collect();
    let weighted_index =
        WeightedIndex::new(weights).map_err(|_| RoutingError::VolumeSplitFailed)?;
    let mut rng = rand::thread_rng();
    let idx = weighted_index.sample(&mut rng);
    splits
        .get(idx)
        .map(|split| split.output.clone())
        .ok_or(RoutingError::VolumeSplitFailed)
}

pub fn evaluate_output(output: &Output) -> RoutingResult<(Vec<ConnectorInfo>, Vec<ConnectorInfo>)> {
    match output {
        Output::Priority(connectors) => {
            let first_connector = connectors.first().cloned();
            Ok((
                connectors.clone(),
                vec![first_connector.unwrap_or_default()],
            ))
        }
        Output::VolumeSplit(splits) => {
            let selected_connector = perform_volume_split(splits.clone())?;
            Ok((vec![selected_connector.clone()], vec![selected_connector]))
        }
        Output::VolumeSplitPriority(splits) => {
            let selected_list = perform_volume_split_priority(splits.clone())?;
            Ok((selected_list.clone(), selected_list))
        }
    }
}
