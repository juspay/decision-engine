use std::collections::{HashMap, HashSet};

use super::{
    ast::{ComparisonType, ConnectorInfo, ValueType},
    cgraph::{
        self, Clause, ConstraintGraph, ConstraintGraphData, Edge, EdgeId, EntityId, Node, NodeId,
        NodeKind, NodeValue, Relation, Strength,
    },
    types::TomlConfig,
};
use crate::{
    config::{ConnectorFilters, CurrencyCountryFlowFilter},
    logger,
};

pub const PM_FILTER_DEFAULT_OUTPUT_SENTINEL: &str = "__pm_default__";

#[derive(Debug, Clone)]
pub struct PmFilterGraphBundle {
    pub graph: ConstraintGraph,
    pub explicit_connectors: HashSet<String>,
    pub has_default_rules: bool,
    pub explicit_connector_payment_method_types: HashMap<String, HashSet<String>>,
    pub default_payment_method_types: HashSet<String>,
    pub billing_country_to_iso2: HashMap<String, String>,
    pub node_count: usize,
    pub edge_count: usize,
}

pub fn build_pm_filter_graph_bundle(
    pm_filters: &ConnectorFilters,
    routing_config: Option<&TomlConfig>,
) -> Result<PmFilterGraphBundle, String> {
    let mut compiler = GraphCompiler::default();
    let mut explicit_connectors = HashSet::new();
    let mut has_default_rules = false;
    let mut explicit_connector_payment_method_types = HashMap::new();
    let mut default_payment_method_types = HashSet::new();

    for (connector_name, filters_for_connector) in &pm_filters.0 {
        let normalized_connector = connector_name.trim().to_ascii_lowercase();
        let is_default = normalized_connector == "default";
        let output_value = if is_default {
            has_default_rules = true;
            PM_FILTER_DEFAULT_OUTPUT_SENTINEL.to_string()
        } else {
            explicit_connectors.insert(normalized_connector.clone());
            normalized_connector
        };

        let output_node = compiler.add_value_node("output", &output_value);
        for (payment_method_type, filter) in &filters_for_connector.0 {
            let normalized_payment_method_type = payment_method_type.trim().to_ascii_lowercase();
            if normalized_payment_method_type.is_empty() {
                continue;
            }

            if is_default {
                default_payment_method_types.insert(normalized_payment_method_type.clone());
            } else {
                explicit_connector_payment_method_types
                    .entry(output_value.clone())
                    .or_insert_with(HashSet::new)
                    .insert(normalized_payment_method_type.clone());
            }

            let rule_node = compiler.add_aggregator_node(NodeKind::AllAggregator);
            let pm_type_node =
                compiler.add_value_node("payment_method_type", &normalized_payment_method_type);
            compiler.add_edge(
                pm_type_node,
                rule_node,
                Strength::Strong,
                Relation::Positive,
            );

            attach_country_filter_nodes(&mut compiler, filter, rule_node);
            attach_currency_filter_nodes(&mut compiler, filter, rule_node);
            attach_not_available_flow_nodes(&mut compiler, filter, rule_node);

            compiler.add_edge(rule_node, output_node, Strength::Normal, Relation::Positive);
        }
    }

    let node_count = compiler.nodes.len();
    let edge_count = compiler.edges.len();
    let graph =
        ConstraintGraph::try_from(ConstraintGraphData::new(compiler.nodes, compiler.edges))?;

    let billing_country_to_iso2 = build_billing_country_to_iso2_map(routing_config);

    Ok(PmFilterGraphBundle {
        graph,
        explicit_connectors,
        has_default_rules,
        explicit_connector_payment_method_types,
        default_payment_method_types,
        billing_country_to_iso2,
        node_count,
        edge_count,
    })
}

pub fn has_payment_method_type(parameters: &HashMap<String, Option<ValueType>>) -> bool {
    get_parameter_string(parameters, "payment_method_type").is_some()
}

pub fn filter_eligible_connectors(
    bundle: &PmFilterGraphBundle,
    parameters: &HashMap<String, Option<ValueType>>,
    connectors: &[ConnectorInfo],
) -> Vec<ConnectorInfo> {
    let Some(base_ctx) = build_pm_filter_context(parameters, &bundle.billing_country_to_iso2)
    else {
        return connectors.to_vec();
    };

    connectors
        .iter()
        .filter(|connector| connector_is_eligible(bundle, &base_ctx, connector))
        .cloned()
        .collect()
}

fn connector_is_eligible(
    bundle: &PmFilterGraphBundle,
    base_ctx: &HashMap<String, ValueType>,
    connector: &ConnectorInfo,
) -> bool {
    let normalized_connector = connector.gateway_name.trim().to_ascii_lowercase();
    let payment_method_type = get_context_enum_value(base_ctx, "payment_method_type");
    let explicit_has_pmt_rule = payment_method_type.is_some_and(|pmt| {
        bundle
            .explicit_connector_payment_method_types
            .get(&normalized_connector)
            .is_some_and(|types| types.contains(pmt))
    });
    let default_has_pmt_rule =
        payment_method_type.is_some_and(|pmt| bundle.default_payment_method_types.contains(pmt));

    let (output_clause_value, source) = if explicit_has_pmt_rule {
        (normalized_connector, "explicit")
    } else if default_has_pmt_rule {
        (PM_FILTER_DEFAULT_OUTPUT_SENTINEL.to_string(), "default")
    } else {
        logger::debug!(
            connector = %connector.gateway_name,
            payment_method_type = ?payment_method_type,
            has_explicit_connector = bundle.explicit_connectors.contains(&normalized_connector),
            has_default_rules = bundle.has_default_rules,
            "No pm_filters rule for connector/payment_method_type; allowing by default"
        );
        return true;
    };

    let mut ctx_vals = base_ctx.clone();
    ctx_vals.insert(
        "output".to_string(),
        ValueType::EnumVariant(output_clause_value.clone()),
    );

    let clause = Clause {
        key: "output".to_string(),
        comparison: ComparisonType::Equal,
        value: ValueType::EnumVariant(output_clause_value),
    };

    match bundle
        .graph
        .check_clause_validity(clause, &cgraph::CheckCtx { ctx_vals })
    {
        Ok(true) => true,
        Ok(false) => {
            logger::debug!(
                connector = %connector.gateway_name,
                rule_source = source,
                "Connector filtered by pm_filters constraint graph"
            );
            false
        }
        Err(err) => {
            logger::error!(
                error = ?err,
                connector = %connector.gateway_name,
                "pm_filters graph evaluation failed, failing open for connector"
            );
            true
        }
    }
}

fn get_context_enum_value<'a>(ctx: &'a HashMap<String, ValueType>, key: &str) -> Option<&'a str> {
    ctx.get(key).and_then(|value| match value {
        ValueType::EnumVariant(inner) => Some(inner.as_str()),
        ValueType::StrValue(inner) => Some(inner.as_str()),
        _ => None,
    })
}

fn build_pm_filter_context(
    parameters: &HashMap<String, Option<ValueType>>,
    billing_country_to_iso2: &HashMap<String, String>,
) -> Option<HashMap<String, ValueType>> {
    let payment_method_type = get_parameter_string(parameters, "payment_method_type")
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.to_ascii_lowercase())?;

    let mut ctx_vals = HashMap::new();
    ctx_vals.insert(
        "payment_method_type".to_string(),
        ValueType::EnumVariant(payment_method_type),
    );

    if let Some(currency) = get_parameter_string(parameters, "currency")
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.to_ascii_uppercase())
    {
        ctx_vals.insert("currency".to_string(), ValueType::EnumVariant(currency));
    }

    if let Some(capture_method) = get_parameter_string(parameters, "capture_method")
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.to_ascii_lowercase())
    {
        ctx_vals.insert(
            "capture_method".to_string(),
            ValueType::EnumVariant(capture_method),
        );
    }

    if let Some(billing_country) = get_parameter_string(parameters, "billing_country")
        .and_then(|country| normalize_billing_country_iso2(country, billing_country_to_iso2))
    {
        ctx_vals.insert(
            "billing_country".to_string(),
            ValueType::EnumVariant(billing_country),
        );
    }

    Some(ctx_vals)
}

fn normalize_billing_country_iso2(
    billing_country: &str,
    billing_country_to_iso2: &HashMap<String, String>,
) -> Option<String> {
    let trimmed = billing_country.trim();
    if trimmed.is_empty() {
        return None;
    }

    let normalized = trimmed.to_ascii_uppercase();
    if normalized.len() == 2 && normalized.chars().all(|c| c.is_ascii_alphabetic()) {
        return Some(normalized);
    }

    billing_country_to_iso2.get(trimmed).cloned().or_else(|| {
        billing_country_to_iso2
            .get(&trimmed.to_ascii_lowercase())
            .cloned()
    })
}

fn build_billing_country_to_iso2_map(
    routing_config: Option<&TomlConfig>,
) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let Some(routing_config) = routing_config else {
        return map;
    };

    let Some(billing_country_key_config) = routing_config.keys.keys.get("billing_country") else {
        return map;
    };

    let Some(billing_values) = billing_country_key_config.values.as_ref() else {
        return map;
    };

    for billing_country in parse_csv_values(billing_values) {
        let iso2 = billing_country.trim().to_ascii_uppercase();
        if iso2.len() == 2 && iso2.chars().all(|c| c.is_ascii_alphabetic()) {
            map.insert(billing_country.clone(), iso2.clone());
            map.insert(billing_country.to_ascii_lowercase(), iso2);
        }
    }
    map
}

fn attach_country_filter_nodes(
    compiler: &mut GraphCompiler,
    filter: &CurrencyCountryFlowFilter,
    parent_node: NodeId,
) {
    let Some(allowed_countries) = filter.country.as_ref() else {
        return;
    };

    let allowed_countries = allowed_countries
        .iter()
        .map(|value| value.trim().to_ascii_uppercase())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();

    if allowed_countries.is_empty() {
        return;
    }

    let country_any_node = compiler.add_aggregator_node(NodeKind::AnyAggregator);
    for country in allowed_countries {
        let country_node = compiler.add_value_node("billing_country", &country);
        compiler.add_edge(
            country_node,
            country_any_node,
            Strength::Weak,
            Relation::Positive,
        );
    }

    compiler.add_edge(
        country_any_node,
        parent_node,
        Strength::Normal,
        Relation::Positive,
    );
}

fn attach_currency_filter_nodes(
    compiler: &mut GraphCompiler,
    filter: &CurrencyCountryFlowFilter,
    parent_node: NodeId,
) {
    let Some(allowed_currencies) = filter.currency.as_ref() else {
        return;
    };

    let allowed_currencies = allowed_currencies
        .iter()
        .map(|value| value.trim().to_ascii_uppercase())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();

    if allowed_currencies.is_empty() {
        return;
    }

    let currency_any_node = compiler.add_aggregator_node(NodeKind::AnyAggregator);
    for currency in allowed_currencies {
        let currency_node = compiler.add_value_node("currency", &currency);
        compiler.add_edge(
            currency_node,
            currency_any_node,
            Strength::Weak,
            Relation::Positive,
        );
    }

    compiler.add_edge(
        currency_any_node,
        parent_node,
        Strength::Normal,
        Relation::Positive,
    );
}

fn attach_not_available_flow_nodes(
    compiler: &mut GraphCompiler,
    filter: &CurrencyCountryFlowFilter,
    parent_node: NodeId,
) {
    let Some(capture_method) = filter
        .not_available_flows
        .as_ref()
        .and_then(|flows| flows.capture_method.as_ref())
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
    else {
        return;
    };

    let capture_method_node = compiler.add_value_node("capture_method", &capture_method);
    compiler.add_edge(
        capture_method_node,
        parent_node,
        Strength::Strong,
        Relation::Negative,
    );
}

fn get_parameter_string<'a>(
    parameters: &'a HashMap<String, Option<ValueType>>,
    key: &str,
) -> Option<&'a str> {
    parameters.get(key).and_then(|value| match value.as_ref() {
        Some(ValueType::EnumVariant(inner)) => Some(inner.as_str()),
        Some(ValueType::StrValue(inner)) => Some(inner.as_str()),
        _ => None,
    })
}

fn parse_csv_values(raw_values: &str) -> Vec<String> {
    raw_values
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

#[derive(Default)]
struct GraphCompiler {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    value_nodes: HashMap<NodeValue, NodeId>,
}

impl GraphCompiler {
    fn add_value_node(&mut self, key: &str, value: &str) -> NodeId {
        let node_value = NodeValue::Value(Clause {
            key: key.to_string(),
            comparison: ComparisonType::Equal,
            value: ValueType::EnumVariant(value.to_string()),
        });

        if let Some(existing_id) = self.value_nodes.get(&node_value).copied() {
            return existing_id;
        }

        let node_id = self.add_node(NodeKind::Value(node_value.clone()));
        self.value_nodes.insert(node_value, node_id);
        node_id
    }

    fn add_aggregator_node(&mut self, kind: NodeKind) -> NodeId {
        self.add_node(kind)
    }

    fn add_node(&mut self, kind: NodeKind) -> NodeId {
        let node_id = NodeId::with_id(self.nodes.len());
        self.nodes.push(Node {
            kind,
            preds: Vec::new(),
            succs: Vec::new(),
        });
        node_id
    }

    fn add_edge(&mut self, pred: NodeId, succ: NodeId, strength: Strength, relation: Relation) {
        let edge_id = EdgeId::with_id(self.edges.len());
        self.edges.push(Edge {
            strength,
            relation,
            pred,
            succ,
        });
        self.nodes[pred.get_id()].succs.push(edge_id);
        self.nodes[succ.get_id()].preds.push(edge_id);
    }
}
