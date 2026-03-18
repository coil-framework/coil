use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum A11yError {
    EmptyField { field: &'static str },
    InvalidId { field: &'static str, value: String },
    InvalidContrastRatio { field: &'static str, ratio: f32 },
    MissingLabel { field_id: String },
    MissingCaption { table_id: String },
}

impl fmt::Display for A11yError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(f, "`{field}` cannot be empty"),
            Self::InvalidId { field, value } => {
                write!(f, "`{field}` contains an invalid id `{value}`")
            }
            Self::InvalidContrastRatio { field, ratio } => {
                write!(f, "`{field}` must be at least 0 and finite, got `{ratio}`")
            }
            Self::MissingLabel { field_id } => {
                write!(f, "form field `{field_id}` is missing an accessible label")
            }
            Self::MissingCaption { table_id } => {
                write!(f, "table `{table_id}` is missing a caption")
            }
        }
    }
}

impl Error for A11yError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormFieldContract {
    pub field_id: String,
    pub label_id: String,
    pub description_id: Option<String>,
    pub error_id: Option<String>,
    pub has_visible_label: bool,
}

impl FormFieldContract {
    pub fn new(
        field_id: impl Into<String>,
        label_id: impl Into<String>,
        description_id: Option<String>,
        error_id: Option<String>,
        has_visible_label: bool,
    ) -> Result<Self, A11yError> {
        let field_id = validate_id("field_id", field_id.into())?;
        let label_id = validate_id("label_id", label_id.into())?;
        if !has_visible_label {
            return Err(A11yError::MissingLabel { field_id });
        }

        Ok(Self {
            field_id,
            label_id,
            description_id: description_id
                .map(|value| validate_id("description_id", value))
                .transpose()?,
            error_id: error_id
                .map(|value| validate_id("error_id", value))
                .transpose()?,
            has_visible_label,
        })
    }

    pub fn described_by(&self) -> Vec<&str> {
        let mut ids = Vec::new();
        if let Some(description_id) = &self.description_id {
            ids.push(description_id.as_str());
        }
        if let Some(error_id) = &self.error_id {
            ids.push(error_id.as_str());
        }
        ids
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorSummary {
    pub summary_id: String,
    pub title: String,
    pub field_ids: Vec<String>,
}

impl ErrorSummary {
    pub fn new(
        summary_id: impl Into<String>,
        title: impl Into<String>,
        field_ids: Vec<String>,
    ) -> Result<Self, A11yError> {
        Ok(Self {
            summary_id: validate_id("summary_id", summary_id.into())?,
            title: require_non_empty("summary_title", title.into())?,
            field_ids: field_ids
                .into_iter()
                .map(|field_id| validate_id("summary_field_id", field_id))
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableContract {
    pub table_id: String,
    pub caption: String,
    pub row_header_column: Option<String>,
    pub sortable_columns: Vec<String>,
}

impl TableContract {
    pub fn new(
        table_id: impl Into<String>,
        caption: impl Into<String>,
        row_header_column: Option<String>,
        sortable_columns: Vec<String>,
    ) -> Result<Self, A11yError> {
        let table_id = validate_id("table_id", table_id.into())?;
        let caption = require_non_empty("table_caption", caption.into())?;
        if caption.trim().is_empty() {
            return Err(A11yError::MissingCaption { table_id });
        }

        Ok(Self {
            table_id,
            caption,
            row_header_column,
            sortable_columns,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialogContract {
    pub dialog_id: String,
    pub title_id: String,
    pub description_id: Option<String>,
    pub initial_focus_target: String,
    pub restore_focus_target: String,
}

impl DialogContract {
    pub fn new(
        dialog_id: impl Into<String>,
        title_id: impl Into<String>,
        description_id: Option<String>,
        initial_focus_target: impl Into<String>,
        restore_focus_target: impl Into<String>,
    ) -> Result<Self, A11yError> {
        Ok(Self {
            dialog_id: validate_id("dialog_id", dialog_id.into())?,
            title_id: validate_id("dialog_title_id", title_id.into())?,
            description_id: description_id
                .map(|value| validate_id("dialog_description_id", value))
                .transpose()?,
            initial_focus_target: validate_id("dialog_initial_focus", initial_focus_target.into())?,
            restore_focus_target: validate_id("dialog_restore_focus", restore_focus_target.into())?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationContract {
    pub skip_link_target: String,
    pub main_landmark_id: String,
    pub breadcrumb_nav_id: Option<String>,
}

impl NavigationContract {
    pub fn standard() -> Self {
        Self {
            skip_link_target: "main-content".to_string(),
            main_landmark_id: "main-content".to_string(),
            breadcrumb_nav_id: Some("breadcrumbs".to_string()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FragmentFocusMode {
    Preserve,
    MoveToHeading,
    MoveToErrorSummary,
    MoveToCustomTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FragmentUpdateContract {
    pub target_id: String,
    pub focus_mode: FragmentFocusMode,
    pub custom_focus_target: Option<String>,
    pub live_region_id: Option<String>,
    pub status_message: Option<String>,
}

impl FragmentUpdateContract {
    pub fn new(
        target_id: impl Into<String>,
        focus_mode: FragmentFocusMode,
        custom_focus_target: Option<String>,
        live_region_id: Option<String>,
        status_message: Option<String>,
    ) -> Result<Self, A11yError> {
        Ok(Self {
            target_id: validate_id("fragment_target_id", target_id.into())?,
            focus_mode,
            custom_focus_target: custom_focus_target
                .map(|value| validate_id("fragment_focus_target", value))
                .transpose()?,
            live_region_id: live_region_id
                .map(|value| validate_id("live_region_id", value))
                .transpose()?,
            status_message: status_message.map(|message| message.trim().to_string()),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveRegionAnnouncement {
    pub region_id: String,
    pub message: String,
    pub atomic: bool,
}

impl LiveRegionAnnouncement {
    pub fn new(
        region_id: impl Into<String>,
        message: impl Into<String>,
        atomic: bool,
    ) -> Result<Self, A11yError> {
        Ok(Self {
            region_id: validate_id("region_id", region_id.into())?,
            message: require_non_empty("announcement_message", message.into())?,
            atomic,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ThemeAccessibilityContract {
    pub body_text_contrast: f32,
    pub muted_text_contrast: f32,
    pub focus_indicator_contrast: f32,
    pub honors_reduced_motion: bool,
    pub visible_focus_states: bool,
}

impl ThemeAccessibilityContract {
    pub fn new(
        body_text_contrast: f32,
        muted_text_contrast: f32,
        focus_indicator_contrast: f32,
        honors_reduced_motion: bool,
        visible_focus_states: bool,
    ) -> Result<Self, A11yError> {
        validate_ratio("body_text_contrast", body_text_contrast)?;
        validate_ratio("muted_text_contrast", muted_text_contrast)?;
        validate_ratio("focus_indicator_contrast", focus_indicator_contrast)?;

        Ok(Self {
            body_text_contrast,
            muted_text_contrast,
            focus_indicator_contrast,
            honors_reduced_motion,
            visible_focus_states,
        })
    }

    pub fn meets_platform_baseline(&self) -> bool {
        self.body_text_contrast >= 4.5
            && self.muted_text_contrast >= 3.0
            && self.focus_indicator_contrast >= 3.0
            && self.honors_reduced_motion
            && self.visible_focus_states
    }
}

fn validate_ratio(field: &'static str, ratio: f32) -> Result<(), A11yError> {
    if ratio.is_finite() && ratio >= 0.0 {
        Ok(())
    } else {
        Err(A11yError::InvalidContrastRatio { field, ratio })
    }
}

fn validate_id(field: &'static str, value: String) -> Result<String, A11yError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(A11yError::EmptyField { field });
    }

    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | ':'))
    {
        Ok(trimmed.to_string())
    } else {
        Err(A11yError::InvalidId {
            field,
            value: trimmed.to_string(),
        })
    }
}

fn require_non_empty(field: &'static str, value: String) -> Result<String, A11yError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(A11yError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_field_contract_wires_description_and_error_ids() {
        let field = FormFieldContract::new(
            "email",
            "email-label",
            Some("email-help".to_string()),
            Some("email-error".to_string()),
            true,
        )
        .unwrap();

        assert_eq!(field.described_by(), vec!["email-help", "email-error"]);
    }

    #[test]
    fn table_and_dialog_contracts_require_accessible_identifiers() {
        let table = TableContract::new(
            "orders-table",
            "Customer orders",
            Some("order-number".to_string()),
            vec!["status".to_string(), "total".to_string()],
        )
        .unwrap();
        let dialog = DialogContract::new(
            "refund-dialog",
            "refund-dialog-title",
            Some("refund-dialog-body".to_string()),
            "refund-amount",
            "refund-button",
        )
        .unwrap();

        assert_eq!(table.caption, "Customer orders");
        assert_eq!(dialog.initial_focus_target, "refund-amount");
    }

    #[test]
    fn fragment_updates_carry_focus_and_live_region_rules() {
        let fragment = FragmentUpdateContract::new(
            "booking-panel",
            FragmentFocusMode::MoveToErrorSummary,
            None,
            Some("booking-status".to_string()),
            Some("Availability updated".to_string()),
        )
        .unwrap();
        let announcement =
            LiveRegionAnnouncement::new("booking-status", "Availability updated", true).unwrap();

        assert_eq!(fragment.focus_mode, FragmentFocusMode::MoveToErrorSummary);
        assert_eq!(announcement.region_id, "booking-status");
        assert!(announcement.atomic);
    }

    #[test]
    fn theme_contract_detects_platform_baseline_failures() {
        let good = ThemeAccessibilityContract::new(4.8, 3.2, 3.5, true, true).unwrap();
        let poor = ThemeAccessibilityContract::new(2.8, 2.2, 2.0, false, false).unwrap();

        assert!(good.meets_platform_baseline());
        assert!(!poor.meets_platform_baseline());
    }

    #[test]
    fn fields_without_visible_labels_are_rejected() {
        let error =
            FormFieldContract::new("search", "search-label", None, None, false).unwrap_err();

        assert_eq!(
            error,
            A11yError::MissingLabel {
                field_id: "search".to_string()
            }
        );
    }
}
