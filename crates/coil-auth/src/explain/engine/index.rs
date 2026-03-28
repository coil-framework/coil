use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct GraphNode {
    pub(super) object: Object,
    pub(super) relation: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct ExplainIndex {
    tuples_by_node: HashMap<GraphNode, Vec<Tuple>>,
}

impl ExplainIndex {
    pub(super) fn new(tuples: &[Tuple]) -> Self {
        let mut tuples_by_node = HashMap::new();

        for tuple in tuples {
            let node = GraphNode {
                object: tuple.object.clone(),
                relation: Some(tuple.relation.clone()),
            };
            tuples_by_node
                .entry(node)
                .or_insert_with(Vec::new)
                .push(tuple.clone());
        }

        Self { tuples_by_node }
    }

    pub(super) fn tuples_for(&self, node: &GraphNode) -> &[Tuple] {
        self.tuples_by_node
            .get(node)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

pub(super) enum Evaluation {
    Allowed(Vec<ExplainStep>),
    Denied(DeniedExplanation),
}
