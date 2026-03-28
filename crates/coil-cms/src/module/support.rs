use super::*;

pub(super) fn cms_live_pages_repository() -> DataRepositoryContribution {
    DataRepositoryContribution::new(
        RepositorySpec::new(
            "cms.pages.live",
            TableName::new("cms_pages").expect("constant cms table is valid"),
            vec![
                QueryField::new("page_id").expect("constant cms field is valid"),
                QueryField::new("title").expect("constant cms field is valid"),
                QueryField::new("live_path").expect("constant cms field is valid"),
                QueryField::new("updated_at").expect("constant cms field is valid"),
            ],
        )
        .expect("constant cms repository is valid")
        .with_locale_field("locale")
        .expect("constant cms locale field is valid")
        .with_publication_field("workflow_status", "published")
        .expect("constant cms publication field is valid")
        .with_filterable_field("slug")
        .expect("constant cms filter field is valid")
        .with_sortable_field("live_path")
        .expect("constant cms sortable field is valid")
        .with_default_sort(QuerySort::ascending("live_path").expect("constant cms sort is valid")),
        DataRepositoryQueryProfile::new(
            PageRequest::new(0, 24).expect("constant cms page size is valid"),
            PublicationVisibility::PublishedOnly,
            QueryCacheScope::Public,
        )
        .with_localized_cache_scope(QueryCacheScope::LocaleScoped),
    )
}

pub(super) fn default_retry_policy() -> RetryPolicy {
    RetryPolicy::new(3, Duration::from_secs(15), Duration::from_secs(300))
        .expect("constant retry policy is valid")
}
