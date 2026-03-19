use super::*;

#[derive(Debug)]
pub enum DavendaAuthError {
    Rebac(RebacError),
    MissingCapabilityBinding {
        capability: Capability,
    },
    ResourceNamespaceMismatch {
        capability: Capability,
        actual: Namespace,
        expected: Vec<Namespace>,
    },
    UnsupportedExplainNamespace {
        namespace: String,
    },
    UnsupportedExplainRelation {
        relation: String,
    },
}

impl fmt::Display for DavendaAuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rebac(error) => write!(f, "{error}"),
            Self::MissingCapabilityBinding { capability } => {
                write!(f, "no binding exists for capability `{capability}`")
            }
            Self::ResourceNamespaceMismatch {
                capability,
                actual,
                expected,
            } => {
                let expected = expected
                    .iter()
                    .map(Namespace::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(
                    f,
                    "capability `{capability}` does not apply to `{actual}` resources; expected one of [{expected}]"
                )
            }
            Self::UnsupportedExplainNamespace { namespace } => {
                write!(
                    f,
                    "cannot explain tuples or schema entries for unsupported namespace `{namespace}`"
                )
            }
            Self::UnsupportedExplainRelation { relation } => {
                write!(
                    f,
                    "cannot explain tuples or schema entries for unsupported relation `{relation}`"
                )
            }
        }
    }
}

impl Error for DavendaAuthError {}

impl From<RebacError> for DavendaAuthError {
    fn from(value: RebacError) -> Self {
        Self::Rebac(value)
    }
}
