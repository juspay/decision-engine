use std::{collections::HashMap, marker::PhantomData};

use serde::{Deserialize, Serialize};

use crate::euclid::ast::{ComparisonType, ValueType};

#[derive(Debug, thiserror::Error)]
pub enum CgraphError {
    #[error("operation performed on incompatible data types")]
    InvalidOperation,
    #[error("error fetching node")]
    NodeNotFound,
    #[error("edge not found")]
    EdgeNotFound,
    #[error("Checking failed")]
    CheckFailure(AnalysisTrace),
}

impl CgraphError {
    fn get_trace(self) -> Result<AnalysisTrace, Self> {
        match self {
            Self::CheckFailure(trace) => Ok(trace),
            err => Err(err),
        }
    }
}

#[derive(Debug)]
pub enum ValueTracePredecessor {
    Mandatory(Box<AnalysisTrace>),
    OneOf(Vec<AnalysisTrace>),
}

#[derive(Debug)]
pub enum AnalysisTrace {
    Value {
        value: NodeValue,
        relation: Relation,
        predecessors: Option<ValueTracePredecessor>,
    },

    AllAggregation {
        unsatisfied: Vec<AnalysisTrace>,
    },

    AnyAggregation {
        unsatisfied: Vec<AnalysisTrace>,
    },
}

pub trait EntityId: Sized + Copy + PartialEq + Eq {
    fn get_id(&self) -> usize;
    fn with_id(id: usize) -> Self;
}

macro_rules! make_entity_id {
    ($name: ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(usize);

        impl EntityId for $name {
            fn get_id(&self) -> usize {
                self.0
            }

            fn with_id(id: usize) -> Self {
                Self(id)
            }
        }
    };
}

make_entity_id!(NodeId);
make_entity_id!(EdgeId);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DenseMap<K, V> {
    data: Vec<V>,
    #[serde(skip)]
    _marker: PhantomData<K>,
}

impl<K, V> Default for DenseMap<K, V> {
    fn default() -> Self {
        Self {
            data: Vec::default(),
            _marker: PhantomData,
        }
    }
}

impl<K: EntityId, V> DenseMap<K, V> {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            _marker: PhantomData,
        }
    }

    pub fn get(&self, id: K) -> Option<&V> {
        self.data.get(id.get_id())
    }

    pub fn iter(&self) -> impl Iterator<Item = (K, &V)> {
        self.data
            .iter()
            .enumerate()
            .map(|(i, d)| (K::with_id(i), d))
    }
}

impl<K, V> From<Vec<V>> for DenseMap<K, V> {
    fn from(value: Vec<V>) -> Self {
        Self {
            data: value,
            _marker: PhantomData,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Clause {
    pub key: String,
    pub comparison: ComparisonType,
    pub value: ValueType,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum NodeValue {
    Key(String),
    Value(Clause),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum NodeKind {
    AllAggregator,
    AnyAggregator,
    Value(NodeValue),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub kind: NodeKind,
    pub preds: Vec<EdgeId>,
    pub succs: Vec<EdgeId>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Strength {
    Weak,
    Normal,
    Strong,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Relation {
    Positive,
    Negative,
}

impl From<Relation> for bool {
    fn from(value: Relation) -> Self {
        match value {
            Relation::Positive => true,
            Relation::Negative => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub strength: Strength,
    pub relation: Relation,
    pub pred: NodeId,
    pub succ: NodeId,
}

pub struct CheckCtx {
    pub ctx_vals: HashMap<String, ValueType>,
}

impl From<HashMap<String, Option<ValueType>>> for CheckCtx {
    fn from(value: HashMap<String, Option<ValueType>>) -> Self {
        let mut ctx_vals = HashMap::new();
        for (key, maybe_value) in value {
            if let Some(value) = maybe_value {
                ctx_vals.insert(key, value);
            }
        }

        Self { ctx_vals }
    }
}

impl CheckCtx {
    fn check(&self, value: &NodeValue, strength: Strength) -> Result<bool, CgraphError> {
        match value {
            NodeValue::Key(k) => {
                Ok(self.ctx_vals.contains_key(k) || matches!(strength, Strength::Weak))
            }

            NodeValue::Value(comp) => {
                use ComparisonType::*;
                use ValueType::{EnumVariant, MetadataVariant, Number, StrValue};
                let Some(ctx_value) = self.ctx_vals.get(&comp.key) else {
                    return Ok(matches!(strength, Strength::Weak));
                };

                Ok(match (ctx_value, &comp.comparison, &comp.value) {
                    (EnumVariant(e1), Equal, EnumVariant(e2)) => e1 == e2,
                    (EnumVariant(e1), NotEqual, EnumVariant(e2)) => e1 != e2,
                    (Number(n1), Equal, Number(n2)) => n1 == n2,
                    (Number(n1), NotEqual, Number(n2)) => n1 != n2,
                    (Number(n1), GreaterThanEqual, Number(n2)) => n1 >= n2,
                    (Number(n1), LessThanEqual, Number(n2)) => n1 <= n2,
                    (Number(n1), GreaterThan, Number(n2)) => n1 > n2,
                    (Number(n1), LessThan, Number(n2)) => n1 < n2,
                    (MetadataVariant(m1), Equal, MetadataVariant(m2)) => m1 == m2,
                    (MetadataVariant(m1), NotEqual, MetadataVariant(m2)) => m1 != m2,
                    (StrValue(s1), Equal, StrValue(s2)) => s1 == s2,
                    (StrValue(s1), NotEqual, StrValue(s2)) => s1 == s2,
                    _ => return Err(CgraphError::InvalidOperation),
                })
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintGraphData {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
}

impl TryFrom<ConstraintGraphData> for ConstraintGraph {
    type Error = String;

    fn try_from(value: ConstraintGraphData) -> Result<Self, Self::Error> {
        let nodes: DenseMap<NodeId, Node> = value.nodes.into();
        let edges: DenseMap<EdgeId, Edge> = value.edges.into();
        let mut value_map: HashMap<NodeValue, NodeId> = HashMap::new();

        for (node_id, node) in nodes.iter() {
            let NodeKind::Value(ref value) = node.kind else {
                continue;
            };

            if value_map.contains_key(value) {
                return Err("duplicate key/value node found".to_string());
            }

            value_map.insert(value.clone(), node_id);
        }

        Ok(Self {
            nodes,
            edges,
            value_map,
        })
    }
}

impl From<ConstraintGraph> for ConstraintGraphData {
    fn from(value: ConstraintGraph) -> Self {
        Self {
            nodes: value.nodes.data,
            edges: value.edges.data,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(try_from = "ConstraintGraphData", into = "ConstraintGraphData")]
pub struct ConstraintGraph {
    nodes: DenseMap<NodeId, Node>,
    edges: DenseMap<EdgeId, Edge>,
    value_map: HashMap<NodeValue, NodeId>,
}

impl ConstraintGraph {
    pub fn check_clause_validity(
        &self,
        clause: Clause,
        ctx: &CheckCtx,
    ) -> Result<bool, CgraphError> {
        let Some(&node_id) = self.value_map.get(&NodeValue::Value(clause)) else {
            return Ok(false);
        };

        let result = self.check_node(node_id, ctx, Relation::Positive, Strength::Strong);

        match result {
            Ok(()) => Ok(true),
            Err(e) => {
                e.get_trace()?;
                Ok(false)
            }
        }
    }

    pub fn check_node(
        &self,
        node_id: NodeId,
        ctx: &CheckCtx,
        relation: Relation,
        strength: Strength,
    ) -> Result<(), CgraphError> {
        let Some(node) = self.nodes.get(node_id) else {
            return Err(CgraphError::NodeNotFound);
        };

        match &node.kind {
            NodeKind::AllAggregator => self.validate_all_aggregator(node, ctx),

            NodeKind::AnyAggregator => self.validate_any_aggregator(node, ctx),

            NodeKind::Value(val) => self.validate_value(val, strength, relation, &node.preds, ctx),
        }
    }

    fn validate_all_aggregator(&self, node: &Node, ctx: &CheckCtx) -> Result<(), CgraphError> {
        let mut unsatisfied = Vec::<AnalysisTrace>::new();

        for pred in node.preds.iter().copied() {
            let Some(edge) = self.edges.get(pred) else {
                return Err(CgraphError::EdgeNotFound);
            };

            if let Err(e) = self.check_node(edge.pred, ctx, edge.relation, edge.strength) {
                unsatisfied.push(e.get_trace()?);
            }
        }

        if !unsatisfied.is_empty() {
            Err(CgraphError::CheckFailure(AnalysisTrace::AllAggregation {
                unsatisfied,
            }))
        } else {
            Ok(())
        }
    }

    fn validate_any_aggregator(&self, node: &Node, ctx: &CheckCtx) -> Result<(), CgraphError> {
        let mut unsatisfied = Vec::<AnalysisTrace>::new();
        let mut matched_one = false;

        for pred in node.preds.iter().copied() {
            let Some(edge) = self.edges.get(pred) else {
                return Err(CgraphError::EdgeNotFound);
            };

            if let Err(e) = self.check_node(edge.pred, ctx, edge.relation, edge.strength) {
                unsatisfied.push(e.get_trace()?);
            } else {
                matched_one = true;
            }
        }

        if matched_one || node.preds.is_empty() {
            Ok(())
        } else {
            Err(CgraphError::CheckFailure(AnalysisTrace::AnyAggregation {
                unsatisfied,
            }))
        }
    }

    fn validate_value(
        &self,
        val: &NodeValue,
        strength: Strength,
        relation: Relation,
        preds: &[EdgeId],
        ctx: &CheckCtx,
    ) -> Result<(), CgraphError> {
        let in_context = ctx.check(val, strength)?;
        let relation_bool: bool = relation.into();

        if in_context != relation_bool {
            return Err(CgraphError::CheckFailure(AnalysisTrace::Value {
                value: val.clone(),
                relation,
                predecessors: None,
            }));
        }

        let mut matched_one = false;
        let mut unsatisfied = Vec::<AnalysisTrace>::new();

        for pred in preds.iter().copied() {
            let Some(edge) = self.edges.get(pred) else {
                return Err(CgraphError::EdgeNotFound);
            };

            let result = self.check_node(edge.pred, ctx, edge.relation, edge.strength);

            match (edge.strength, result) {
                (Strength::Strong, Err(err)) => {
                    return Err(CgraphError::CheckFailure(AnalysisTrace::Value {
                        value: val.clone(),
                        relation,
                        predecessors: Some(ValueTracePredecessor::Mandatory(Box::new(
                            err.get_trace()?,
                        ))),
                    }));
                }

                (Strength::Normal | Strength::Weak, Err(err)) => {
                    unsatisfied.push(err.get_trace()?);
                }

                (Strength::Strong | Strength::Normal | Strength::Weak, Ok(_)) => {
                    matched_one = true;
                }
            }
        }

        if matched_one || preds.is_empty() {
            Ok(())
        } else {
            Err(CgraphError::CheckFailure(AnalysisTrace::Value {
                value: val.clone(),
                relation,
                predecessors: Some(ValueTracePredecessor::OneOf(unsatisfied)),
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Serialize, Deserialize)]
    struct Config {
        routing_config: RoutingConfig,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct RoutingConfig {
        constraint_graph: ConstraintGraphData,
    }

    #[test]
    fn get_toml() {
        let node1 = Node {
            kind: NodeKind::Value(NodeValue::Value(Clause {
                key: "payment_method".to_string(),
                comparison: ComparisonType::Equal,
                value: ValueType::EnumVariant("card".to_string()),
            })),
            preds: vec![],
            succs: vec![EdgeId(0)],
        };
        let node2 = Node {
            kind: NodeKind::Value(NodeValue::Value(Clause {
                key: "payment_method".to_string(),
                comparison: ComparisonType::Equal,
                value: ValueType::EnumVariant("bank_debit".to_string()),
            })),
            preds: vec![],
            succs: vec![EdgeId(1)],
        };
        let node3 = Node {
            kind: NodeKind::Value(NodeValue::Value(Clause {
                key: "output".to_string(),
                comparison: ComparisonType::Equal,
                value: ValueType::EnumVariant("stripe".to_string()),
            })),
            preds: vec![EdgeId(0)],
            succs: vec![],
        };
        let node4 = Node {
            kind: NodeKind::Value(NodeValue::Value(Clause {
                key: "output".to_string(),
                comparison: ComparisonType::Equal,
                value: ValueType::EnumVariant("adyen".to_string()),
            })),
            preds: vec![EdgeId(1)],
            succs: vec![],
        };

        let edge1 = Edge {
            strength: Strength::Strong,
            relation: Relation::Positive,
            pred: NodeId(0),
            succ: NodeId(2),
        };
        let edge2 = Edge {
            strength: Strength::Strong,
            relation: Relation::Positive,
            pred: NodeId(1),
            succ: NodeId(3),
        };

        let graph = ConstraintGraphData {
            nodes: vec![node1, node2, node3, node4],
            edges: vec![edge1, edge2],
        };

        let config = Config {
            routing_config: RoutingConfig {
                constraint_graph: graph,
            },
        };

        let string = toml::to_string(&config).expect("failed toml conversion");

        println!("{string}");
    }
}
