use crate::{
    A11yError, DialogContract, FormFieldContract, FragmentFocusMode, FragmentUpdateContract,
    LiveRegionAnnouncement, TableContract, ThemeAccessibilityContract,
};

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
    let error = FormFieldContract::new("search", "search-label", None, None, false).unwrap_err();

    assert_eq!(
        error,
        A11yError::MissingLabel {
            field_id: "search".to_string()
        }
    );
}
