use super::*;

pub const DEFAULT_EXPLAIN_MAX_DEPTH: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExplainOptions {
    pub max_depth: usize,
    pub cycle_protection: bool,
}

impl ExplainOptions {
    pub const fn new(max_depth: usize) -> Self {
        Self {
            max_depth,
            cycle_protection: true,
        }
    }

    pub const fn with_cycle_protection(mut self, cycle_protection: bool) -> Self {
        self.cycle_protection = cycle_protection;
        self
    }

    pub const fn normalized(self) -> Self {
        let max_depth = if self.max_depth == 0 {
            1
        } else {
            self.max_depth
        };

        Self {
            max_depth,
            cycle_protection: self.cycle_protection,
        }
    }
}

impl Default for ExplainOptions {
    fn default() -> Self {
        Self {
            max_depth: DEFAULT_EXPLAIN_MAX_DEPTH,
            cycle_protection: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExplainDecision {
    Allow,
    Deny,
}

impl ExplainDecision {
    pub const fn is_allowed(self) -> bool {
        matches!(self, Self::Allow)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExplainedNode {
    pub object: Entity,
    pub relation: Option<Relation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExplainStep {
    Start {
        node: ExplainedNode,
    },
    DirectSubjectMatch {
        node: ExplainedNode,
    },
    TupleSubjectMatch {
        from: ExplainedNode,
        tuple: DefaultTuple,
    },
    Inherit {
        from: ExplainedNode,
        to: ExplainedNode,
    },
    TupleTraversal {
        from: ExplainedNode,
        tuple: DefaultTuple,
        to: ExplainedNode,
    },
    Computed {
        from: ExplainedNode,
        via_tuple: DefaultTuple,
        to: ExplainedNode,
    },
    TupleToUserset {
        from: ExplainedNode,
        via_tuple: DefaultTuple,
        to: ExplainedNode,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeniedReason {
    NoMatchingPath,
    RecursionLimitReached { max_depth: usize },
    CycleDetected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeniedAttempt {
    Inherit {
        step: ExplainStep,
        result: Box<DeniedExplanation>,
    },
    TupleTraversal {
        step: ExplainStep,
        result: Box<DeniedExplanation>,
    },
    Computed {
        step: ExplainStep,
        result: Box<DeniedExplanation>,
    },
    TupleToUserset {
        step: ExplainStep,
        result: Box<DeniedExplanation>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeniedExplanation {
    pub node: ExplainedNode,
    pub reason: DeniedReason,
    pub attempts: Vec<DeniedAttempt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllowedExplanation {
    pub steps: Vec<ExplainStep>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExplainTrace {
    Allowed(AllowedExplanation),
    Denied(DeniedExplanation),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityExplanation {
    pub manifest: AuthModelManifest,
    pub subject: DefaultSubject,
    pub capability: Capability,
    pub object: Entity,
    pub binding: CapabilityBinding,
    pub decision: ExplainDecision,
    pub options: ExplainOptions,
    pub trace: ExplainTrace,
}
