use davenda_core::{DataRepositoryContribution, DataRepositoryQueryProfile};
use davenda_data::{
    PageRequest, PublicationVisibility, QueryCacheScope, QueryField, QuerySort, RepositorySpec,
    TableName,
};

pub(crate) fn commerce_catalog_products_repository() -> DataRepositoryContribution {
    DataRepositoryContribution::new(
        RepositorySpec::new(
            "commerce.catalog.products",
            TableName::new("davenda.catalog_products").expect("constant commerce table is valid"),
            vec![
                QueryField::new("product_id").expect("constant commerce field is valid"),
                QueryField::new("product_title").expect("constant commerce field is valid"),
                QueryField::new("product_slug").expect("constant commerce field is valid"),
                QueryField::new("updated_at").expect("constant commerce field is valid"),
            ],
        )
        .expect("constant commerce repository is valid")
        .with_locale_field("locale")
        .expect("constant commerce locale field is valid")
        .with_publication_field("catalog_status", "active")
        .expect("constant commerce publication field is valid")
        .with_filterable_field("collection_handle")
        .expect("constant commerce filter field is valid")
        .with_sortable_field("product_title")
        .expect("constant commerce sortable field is valid")
        .with_default_sort(
            QuerySort::ascending("product_title").expect("constant commerce sort is valid"),
        ),
        DataRepositoryQueryProfile::new(
            PageRequest::new(0, 24).expect("constant commerce page size is valid"),
            PublicationVisibility::PublishedOnly,
            QueryCacheScope::Public,
        )
        .with_localized_cache_scope(QueryCacheScope::LocaleScoped),
    )
}
