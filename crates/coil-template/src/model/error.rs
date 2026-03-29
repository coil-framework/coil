use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateModelError {
    EmptyField {
        field: &'static str,
    },
    InvalidToken {
        field: &'static str,
        value: String,
    },
    DuplicateTemplate {
        key: TemplateKey,
    },
    TemplateNotFound {
        name: TemplateName,
    },
    TemplateKindMismatch {
        name: TemplateName,
        expected: TemplateKind,
        actual: TemplateKind,
    },
    MissingValue {
        key: String,
    },
    MissingTranslation {
        key: String,
    },
    MissingSlotFill {
        slot: SlotName,
    },
    TemplateRead {
        path: String,
        message: String,
    },
    ParseError {
        line: usize,
        column: usize,
        message: String,
    },
    RenderModelConflict {
        path: String,
        message: String,
    },
    FragmentCannotRenderLayout {
        name: TemplateName,
    },
    LayoutCannotBeIncludedAsFragment {
        name: TemplateName,
    },
    InvalidElementName {
        tag: String,
    },
    InvalidAttributeName {
        name: String,
    },
    ValueTypeMismatch {
        key: String,
        expected: &'static str,
    },
}

impl fmt::Display for TemplateModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(f, "`{field}` cannot be empty"),
            Self::InvalidToken { field, value } => {
                write!(f, "`{field}` contains an invalid token `{value}`")
            }
            Self::DuplicateTemplate { key } => write!(f, "template `{key}` is already registered"),
            Self::TemplateNotFound { name } => write!(f, "template `{name}` was not found"),
            Self::TemplateKindMismatch {
                name,
                expected,
                actual,
            } => write!(
                f,
                "template `{name}` resolved to kind `{actual}` but `{expected}` was required"
            ),
            Self::MissingValue { key } => write!(f, "render value `{key}` was not provided"),
            Self::MissingTranslation { key } => {
                write!(f, "translation `{key}` was not provided")
            }
            Self::MissingSlotFill { slot } => write!(f, "slot `{slot}` has no fill or fallback"),
            Self::TemplateRead { path, message } => {
                write!(f, "failed to read template `{path}`: {message}")
            }
            Self::ParseError {
                line,
                column,
                message,
            } => write!(f, "template parse error at {line}:{column}: {message}"),
            Self::RenderModelConflict { path, message } => {
                write!(f, "render model conflict at `{path}`: {message}")
            }
            Self::FragmentCannotRenderLayout { name } => {
                write!(
                    f,
                    "layout template `{name}` cannot be rendered as a fragment"
                )
            }
            Self::LayoutCannotBeIncludedAsFragment { name } => {
                write!(
                    f,
                    "layout template `{name}` cannot be included as a fragment"
                )
            }
            Self::InvalidElementName { tag } => write!(f, "invalid element name `{tag}`"),
            Self::InvalidAttributeName { name } => write!(f, "invalid attribute name `{name}`"),
            Self::ValueTypeMismatch { key, expected } => {
                write!(
                    f,
                    "render value `{key}` does not match expected type `{expected}`"
                )
            }
        }
    }
}

impl Error for TemplateModelError {}
