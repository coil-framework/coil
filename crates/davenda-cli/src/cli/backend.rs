use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use zanzibar::{
    CheckRequest, Object, RebacEngine, RebacError, Schema, Subject, Tuple, TupleUpdate,
};

#[derive(Debug, Clone, Default)]
pub(crate) struct MemoryRebacEngine {
    schema: Arc<Mutex<Option<Schema>>>,
    tuples: Arc<Mutex<Vec<Tuple>>>,
}

impl MemoryRebacEngine {
    pub(crate) fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl RebacEngine for MemoryRebacEngine {
    async fn apply_schema(&self, _tenant_id: i64, schema: Schema) -> Result<(), RebacError> {
        let mut slot = self
            .schema
            .lock()
            .map_err(|_| RebacError::Internal("memory rebac engine mutex poisoned".into()))?;
        *slot = Some(schema);
        Ok(())
    }

    async fn write_tuples(
        &self,
        _tenant_id: i64,
        updates: Vec<TupleUpdate>,
    ) -> Result<(), RebacError> {
        let mut tuples = self
            .tuples
            .lock()
            .map_err(|_| RebacError::Internal("memory rebac engine mutex poisoned".into()))?;

        for update in updates {
            match update {
                TupleUpdate::Write(tuple) => {
                    if !tuples.contains(&tuple) {
                        tuples.push(tuple);
                    }
                }
                TupleUpdate::Delete(tuple) => {
                    tuples.retain(|existing| existing != &tuple);
                }
            }
        }

        Ok(())
    }

    async fn read_tuples(
        &self,
        _tenant_id: i64,
        object: Option<Object>,
        relation: Option<String>,
        subject: Option<Subject>,
    ) -> Result<Vec<Tuple>, RebacError> {
        let tuples = self
            .tuples
            .lock()
            .map_err(|_| RebacError::Internal("memory rebac engine mutex poisoned".into()))?;

        Ok(tuples
            .iter()
            .filter(|tuple| {
                object
                    .as_ref()
                    .is_none_or(|candidate| &tuple.object == candidate)
                    && relation
                        .as_ref()
                        .is_none_or(|candidate| tuple.relation == *candidate)
                    && subject
                        .as_ref()
                        .is_none_or(|candidate| &tuple.subject == candidate)
            })
            .cloned()
            .collect())
    }

    async fn check(
        &self,
        tenant_id: i64,
        subject: &Subject,
        relation: &str,
        object: &Object,
    ) -> Result<bool, RebacError> {
        Ok(!self
            .read_tuples(
                tenant_id,
                Some(object.clone()),
                Some(relation.to_string()),
                Some(subject.clone()),
            )
            .await?
            .is_empty())
    }

    async fn check_many(
        &self,
        tenant_id: i64,
        requests: Vec<CheckRequest>,
    ) -> Result<Vec<bool>, RebacError> {
        let mut results = Vec::with_capacity(requests.len());
        for request in requests {
            results.push(
                self.check(
                    tenant_id,
                    &request.subject,
                    &request.relation,
                    &request.object,
                )
                .await?,
            );
        }
        Ok(results)
    }

    async fn list_objects(
        &self,
        _tenant_id: i64,
        subject: &Subject,
        relation: &str,
        object_namespace: &str,
    ) -> Result<Vec<String>, RebacError> {
        let tuples = self
            .tuples
            .lock()
            .map_err(|_| RebacError::Internal("memory rebac engine mutex poisoned".into()))?;
        let mut values = BTreeSet::new();

        for tuple in tuples.iter() {
            if tuple.object.namespace == object_namespace
                && tuple.relation == relation
                && &tuple.subject == subject
            {
                values.insert(tuple.object.id.clone());
            }
        }

        Ok(values.into_iter().collect())
    }

    async fn list_subjects(
        &self,
        _tenant_id: i64,
        object: &Object,
        relation: &str,
        subject_namespace: &str,
    ) -> Result<Vec<String>, RebacError> {
        let tuples = self
            .tuples
            .lock()
            .map_err(|_| RebacError::Internal("memory rebac engine mutex poisoned".into()))?;
        let mut values = BTreeSet::new();

        for tuple in tuples.iter() {
            if &tuple.object == object
                && tuple.relation == relation
                && tuple.subject.namespace() == subject_namespace
            {
                values.insert(tuple.subject.id().to_string());
            }
        }

        Ok(values.into_iter().collect())
    }
}
