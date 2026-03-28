use super::super::super::support::default_retry_policy;
use super::*;

pub(super) fn search_contributions() -> Vec<SearchIndexContribution> {
    vec![
        SearchIndexContribution::new(
            "search.events",
            SearchDocumentKind::Event,
            SearchVisibility::Public,
            true,
            vec![
                SearchFieldContribution::new("title", "title", SearchFieldRole::Title, true, true),
                SearchFieldContribution::new(
                    "summary",
                    "summary",
                    SearchFieldRole::Summary,
                    false,
                    true,
                ),
                SearchFieldContribution::new(
                    "location",
                    "location",
                    SearchFieldRole::Facet,
                    true,
                    true,
                ),
            ],
            vec![
                SearchInvalidationRule::new(
                    SearchInvalidationTrigger::Published,
                    "event published",
                ),
                SearchInvalidationRule::new(SearchInvalidationTrigger::Updated, "event updated"),
            ],
            SearchRebuildStrategy::Scheduled {
                interval: Duration::from_secs(3600),
            },
        ),
        SearchIndexContribution::new(
            "search.events.bookings",
            SearchDocumentKind::Booking,
            SearchVisibility::Capability(Capability::EventsBookingCheckIn),
            false,
            vec![
                SearchFieldContribution::new(
                    "attendee",
                    "attendee.display_name",
                    SearchFieldRole::Title,
                    true,
                    true,
                ),
                SearchFieldContribution::new(
                    "status",
                    "status",
                    SearchFieldRole::Facet,
                    true,
                    true,
                ),
            ],
            vec![
                SearchInvalidationRule::new(SearchInvalidationTrigger::Updated, "booking changed"),
                SearchInvalidationRule::new(SearchInvalidationTrigger::Deleted, "booking deleted"),
            ],
            SearchRebuildStrategy::ManualOnly,
        ),
    ]
}

pub(super) fn report_definitions() -> Vec<ReportDefinition> {
    vec![ReportDefinition::new(
        "report.events.attendance",
        "Event attendance",
        Some("Attendance and booking-state export for check-in operations".to_string()),
        Capability::EventsBookingCheckIn,
        ReportFormat::Csv,
        ReportSensitivity::Internal,
        ReportDeliveryMode::SignedUrl,
        "reports/events",
        default_retry_policy(),
    )]
}

pub(super) fn bulk_operations() -> Vec<BulkOperationDefinition> {
    vec![BulkOperationDefinition::new(
        "bulk.events.check-in",
        "Bulk check in bookings",
        Some("Applies audited attendance check-in through retry-safe job execution".to_string()),
        Capability::EventsBookingCheckIn,
        BulkOperationKind::CheckIn,
        BulkOperationScope::Events,
        default_retry_policy(),
        Some(1000),
        true,
    )]
}
