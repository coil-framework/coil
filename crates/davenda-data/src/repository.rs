use crate::{
    CompiledQuery, DataModelError, FilterOperator, QueryField, QueryFilter, QuerySort, QuerySpec,
    TableName, compile_filters, ensure_repository_field, quote_identifier, require_non_empty,
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RepositoryContextBindings {
    pub locale_field: Option<QueryField>,
    pub publication_field: Option<QueryField>,
    pub published_value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositorySpec {
    pub id: String,
    pub table: TableName,
    pub projection: Vec<QueryField>,
    pub filterable_fields: Vec<QueryField>,
    pub sortable_fields: Vec<QueryField>,
    pub default_sort: Vec<QuerySort>,
    pub context: RepositoryContextBindings,
}

impl RepositorySpec {
    pub fn new(
        id: impl Into<String>,
        table: TableName,
        projection: Vec<QueryField>,
    ) -> Result<Self, DataModelError> {
        let id = require_non_empty("repository_id", id.into())?;
        if projection.is_empty() {
            return Err(DataModelError::EmptyProjection { repository: id });
        }

        Ok(Self {
            id,
            table,
            filterable_fields: projection.clone(),
            sortable_fields: projection.clone(),
            projection,
            default_sort: Vec::new(),
            context: RepositoryContextBindings::default(),
        })
    }

    pub fn with_filterable_field(
        mut self,
        field: impl Into<String>,
    ) -> Result<Self, DataModelError> {
        let field = QueryField::new(field)?;
        if !self.filterable_fields.contains(&field) {
            self.filterable_fields.push(field);
        }
        Ok(self)
    }

    pub fn with_sortable_field(mut self, field: impl Into<String>) -> Result<Self, DataModelError> {
        let field = QueryField::new(field)?;
        if !self.sortable_fields.contains(&field) {
            self.sortable_fields.push(field);
        }
        Ok(self)
    }

    pub fn with_default_sort(mut self, sort: QuerySort) -> Self {
        self.default_sort.push(sort);
        self
    }

    pub fn with_locale_field(mut self, field: impl Into<String>) -> Result<Self, DataModelError> {
        self.context.locale_field = Some(QueryField::new(field)?);
        Ok(self)
    }

    pub fn with_publication_field(
        mut self,
        field: impl Into<String>,
        published_value: impl Into<String>,
    ) -> Result<Self, DataModelError> {
        self.context.publication_field = Some(QueryField::new(field)?);
        self.context.published_value = Some(require_non_empty(
            "published_value",
            published_value.into(),
        )?);
        Ok(self)
    }

    pub fn compile_query(&self, spec: &QuerySpec) -> Result<CompiledQuery, DataModelError> {
        let mut filters = Vec::new();
        if let (Some(locale), Some(locale_field)) = (
            spec.context.locale.as_ref(),
            self.context.locale_field.as_ref(),
        ) {
            filters.push(QueryFilter::new(
                locale_field.as_str(),
                FilterOperator::Eq,
                vec![locale.clone()],
            )?);
        }

        if let (
            crate::PublicationVisibility::PublishedOnly,
            Some(publication_field),
            Some(published),
        ) = (
            spec.context.publication_visibility,
            self.context.publication_field.as_ref(),
            self.context.published_value.as_ref(),
        ) {
            filters.push(QueryFilter::new(
                publication_field.as_str(),
                FilterOperator::Eq,
                vec![published.clone()],
            )?);
        }

        filters.extend(spec.filters.clone());

        for filter in &filters {
            ensure_repository_field(
                &self.id,
                &filter.field,
                &self.filterable_fields,
                self.context.locale_field.as_ref(),
                self.context.publication_field.as_ref(),
            )?;
        }

        let sort = if spec.sort.is_empty() {
            self.default_sort.clone()
        } else {
            spec.sort.clone()
        };

        for sort_field in &sort {
            ensure_repository_field(
                &self.id,
                &sort_field.field,
                &self.sortable_fields,
                self.context.locale_field.as_ref(),
                self.context.publication_field.as_ref(),
            )?;
        }

        let projection = self
            .projection
            .iter()
            .map(|field| quote_identifier(field.as_str()))
            .collect::<Vec<_>>()
            .join(", ");
        let mut sql = format!(
            "SELECT {projection} FROM {}",
            quote_identifier(self.table.as_str())
        );

        let (where_clauses, bind_values, _) = compile_filters(&filters, 1)?;
        if !where_clauses.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&where_clauses.join(" AND "));
        }

        if !sort.is_empty() {
            sql.push_str(" ORDER BY ");
            sql.push_str(
                &sort
                    .iter()
                    .map(|sort| {
                        format!(
                            "{} {}",
                            quote_identifier(sort.field.as_str()),
                            sort.direction.to_string().to_uppercase()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }

        sql.push_str(&format!(
            " LIMIT {} OFFSET {}",
            spec.page.size,
            spec.page.offset()
        ));

        Ok(CompiledQuery {
            sql,
            bind_values,
            page: spec.page,
            context: spec.context.clone(),
        })
    }
}
