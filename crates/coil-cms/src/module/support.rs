use super::*;

pub(super) fn cms_live_pages_repository() -> DataRepositoryContribution {
    DataRepositoryContribution::new(
        RepositorySpec::new(
            "cms.pages.live",
            TableName::new("cms_pages").expect("constant cms table is valid"),
            vec![
                QueryField::new("page_id").expect("constant cms field is valid"),
                QueryField::new("title").expect("constant cms field is valid"),
                QueryField::new("slug").expect("constant cms field is valid"),
                QueryField::new("template").expect("constant cms field is valid"),
                QueryField::new("summary").expect("constant cms field is valid"),
                QueryField::new("body_html").expect("constant cms field is valid"),
                QueryField::new("content_kind").expect("constant cms field is valid"),
                QueryField::new("block_count").expect("constant cms field is valid"),
                QueryField::new("has_shared_blocks").expect("constant cms field is valid"),
                QueryField::new("page_settings").expect("constant cms field is valid"),
                QueryField::new("show_in_navigation").expect("constant cms field is valid"),
                QueryField::new("allow_indexing").expect("constant cms field is valid"),
                QueryField::new("include_in_sitemap").expect("constant cms field is valid"),
                QueryField::new("navigation_label").expect("constant cms field is valid"),
                QueryField::new("layout_variant").expect("constant cms field is valid"),
                QueryField::new("live_path").expect("constant cms field is valid"),
                QueryField::new("workflow_status").expect("constant cms field is valid"),
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
        .with_filterable_field("content_kind")
        .expect("constant cms filter field is valid")
        .with_filterable_field("has_shared_blocks")
        .expect("constant cms filter field is valid")
        .with_sortable_field("live_path")
        .expect("constant cms sortable field is valid")
        .with_sortable_field("updated_at")
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

pub(super) fn cms_shared_blocks_repository() -> DataRepositoryContribution {
    DataRepositoryContribution::new(
        RepositorySpec::new(
            "cms.shared_blocks",
            TableName::new("cms_shared_blocks").expect("constant cms table is valid"),
            vec![
                QueryField::new("shared_block_id").expect("constant cms field is valid"),
                QueryField::new("label").expect("constant cms field is valid"),
                QueryField::new("block_type_id").expect("constant cms field is valid"),
                QueryField::new("block_payload").expect("constant cms field is valid"),
                QueryField::new("reference_count").expect("constant cms field is valid"),
                QueryField::new("updated_at").expect("constant cms field is valid"),
            ],
        )
        .expect("constant cms repository is valid")
        .with_locale_field("locale")
        .expect("constant cms locale field is valid")
        .with_filterable_field("block_type_id")
        .expect("constant cms filter field is valid")
        .with_sortable_field("label")
        .expect("constant cms sortable field is valid")
        .with_sortable_field("updated_at")
        .expect("constant cms sortable field is valid")
        .with_default_sort(QuerySort::ascending("label").expect("constant cms sort is valid")),
        DataRepositoryQueryProfile::new(
            PageRequest::new(0, 50).expect("constant cms page size is valid"),
            PublicationVisibility::IncludeDrafts,
            QueryCacheScope::UserScoped,
        )
        .with_localized_cache_scope(QueryCacheScope::UserScoped)
        .bind_invocation_principal(),
    )
}

pub(super) fn default_retry_policy() -> RetryPolicy {
    RetryPolicy::new(3, Duration::from_secs(15), Duration::from_secs(300))
        .expect("constant retry policy is valid")
}
