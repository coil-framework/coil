use super::*;

pub(super) fn events_waitlist_repository() -> DataRepositoryContribution {
    DataRepositoryContribution::new(
        RepositorySpec::new(
            "events.waitlist",
            TableName::new("coil.events_waitlist_entries").expect("constant events table is valid"),
            vec![
                QueryField::new("waitlist_entry_id").expect("constant events field is valid"),
                QueryField::new("event_id").expect("constant events field is valid"),
                QueryField::new("slot_id").expect("constant events field is valid"),
                QueryField::new("status").expect("constant events field is valid"),
                QueryField::new("position").expect("constant events field is valid"),
                QueryField::new("created_at").expect("constant events field is valid"),
            ],
        )
        .expect("constant events repository is valid")
        .with_sortable_field("created_at")
        .expect("constant events sortable field is valid")
        .with_default_sort(
            QuerySort::ascending("created_at").expect("constant events sort is valid"),
        )
        .with_filterable_field("event_id")
        .expect("constant events filter field is valid")
        .with_filterable_field("slot_id")
        .expect("constant events filter field is valid"),
        DataRepositoryQueryProfile::new(
            PageRequest::new(0, 50).expect("constant events page size is valid"),
            PublicationVisibility::IncludeDrafts,
            QueryCacheScope::Uncacheable,
        )
        .bind_invocation_principal(),
    )
}

pub(super) fn default_retry_policy() -> RetryPolicy {
    RetryPolicy::new(3, Duration::from_secs(15), Duration::from_secs(300))
        .expect("constant retry policy is valid")
}
