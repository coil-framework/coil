use crate::{DataModelError, DataValue, FilterOperator, QueryField, QueryFilter};

pub(crate) fn ensure_repository_field(
    repository: &str,
    field: &QueryField,
    allowed: &[QueryField],
    locale_field: Option<&QueryField>,
    publication_field: Option<&QueryField>,
) -> Result<(), DataModelError> {
    if allowed.contains(field)
        || locale_field.is_some_and(|allowed_field| allowed_field == field)
        || publication_field.is_some_and(|allowed_field| allowed_field == field)
    {
        Ok(())
    } else {
        Err(DataModelError::UnknownRepositoryField {
            repository: repository.to_string(),
            field: field.to_string(),
        })
    }
}

pub(crate) fn compile_filters(
    filters: &[QueryFilter],
    start_index: usize,
) -> Result<(Vec<String>, Vec<DataValue>, usize), DataModelError> {
    let mut clauses = Vec::new();
    let mut bind_values = Vec::new();
    let mut index = start_index;

    for filter in filters {
        let (clause, values, next_index) = compile_filter(filter, index)?;
        clauses.push(clause);
        bind_values.extend(values);
        index = next_index;
    }

    Ok((clauses, bind_values, index))
}

fn compile_filter(
    filter: &QueryFilter,
    start_index: usize,
) -> Result<(String, Vec<DataValue>, usize), DataModelError> {
    let field = quote_identifier(filter.field.as_str());
    match filter.operator {
        FilterOperator::Eq => {
            ensure_filter_arity(filter, "exactly 1", 1..=1)?;
            Ok((
                format!("{field} = {}", render_placeholder(start_index)),
                vec![DataValue::String(filter.values[0].clone())],
                start_index + 1,
            ))
        }
        FilterOperator::Prefix => {
            ensure_filter_arity(filter, "exactly 1", 1..=1)?;
            Ok((
                format!("{field} LIKE {}", render_placeholder(start_index)),
                vec![DataValue::String(format!("{}%", filter.values[0]))],
                start_index + 1,
            ))
        }
        FilterOperator::Range => {
            ensure_filter_arity(filter, "exactly 2", 2..=2)?;
            Ok((
                format!(
                    "{field} BETWEEN {} AND {}",
                    render_placeholder(start_index),
                    render_placeholder(start_index + 1)
                ),
                vec![
                    DataValue::String(filter.values[0].clone()),
                    DataValue::String(filter.values[1].clone()),
                ],
                start_index + 2,
            ))
        }
        FilterOperator::In => {
            ensure_filter_arity(filter, "at least 1", 1..)?;
            let placeholders = (start_index..start_index + filter.values.len())
                .map(render_placeholder)
                .collect::<Vec<_>>()
                .join(", ");
            Ok((
                format!("{field} IN ({placeholders})"),
                filter
                    .values
                    .iter()
                    .cloned()
                    .map(DataValue::String)
                    .collect(),
                start_index + filter.values.len(),
            ))
        }
    }
}

pub(crate) fn ensure_filter_arity(
    filter: &QueryFilter,
    expected: &'static str,
    range: impl std::ops::RangeBounds<usize>,
) -> Result<(), DataModelError> {
    let actual = filter.values.len();
    let contains = match (range.start_bound(), range.end_bound()) {
        (std::ops::Bound::Included(start), std::ops::Bound::Included(end)) => {
            actual >= *start && actual <= *end
        }
        (std::ops::Bound::Included(start), std::ops::Bound::Unbounded) => actual >= *start,
        _ => false,
    };

    if contains {
        Ok(())
    } else {
        Err(DataModelError::InvalidFilterArity {
            operator: filter.operator,
            expected,
            actual,
        })
    }
}

pub(crate) fn quote_identifier(identifier: &str) -> String {
    identifier
        .split('.')
        .map(|part| format!("\"{}\"", part.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(".")
}

pub(crate) fn render_placeholder(index: usize) -> String {
    format!("${index}")
}
