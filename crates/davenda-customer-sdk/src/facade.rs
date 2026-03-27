use crate::{
    AssetWriteReceipt, AssetWriteRequest, AuditEntry, AuthCheckRequest, AuthCheckResult,
    AuthExplainRequest, AuthExplanation, BackendError, CommerceCatalogCollectionRecord,
    CommerceCatalogCollectionUpdate, CommerceCatalogProductRecord, CommerceCatalogProductUpdate,
    CommerceOrderRecord, CommerceProduct, JobReceipt, JobRequest, ManagedAsset,
    OutboundHttpRequest, OutboundHttpResponse, RepositoryQuery, RepositoryRecordSet,
    RepositoryWrite, RepositoryWriteReceipt,
};
use std::collections::BTreeMap;

pub trait CommerceFacade: Send + Sync {
    fn product(&self, sku: &str) -> Result<Option<CommerceProduct>, BackendError>;

    fn add_order_note(&self, order_id: &str, note: &str) -> Result<(), BackendError>;
}

pub trait JobsFacade: Send + Sync {
    fn enqueue(&self, request: JobRequest) -> Result<JobReceipt, BackendError>;
}

pub trait RepositoryFacade: Send + Sync {
    fn read(&self, query: &RepositoryQuery) -> Result<RepositoryRecordSet, BackendError>;

    fn write(&self, change: RepositoryWrite) -> Result<RepositoryWriteReceipt, BackendError>;
}

pub trait RepositoryFacadeExt: RepositoryFacade {
    fn commerce_catalog_product(
        &self,
        handle_or_sku: &str,
    ) -> Result<Option<CommerceCatalogProductRecord>, BackendError> {
        let records = self.read(
            &RepositoryQuery::new(CommerceCatalogProductRecord::REPOSITORY).with_key(handle_or_sku),
        )?;
        records
            .records
            .first()
            .map(CommerceCatalogProductRecord::from_repository_record)
            .transpose()
    }

    fn update_commerce_catalog_product(
        &self,
        update: &CommerceCatalogProductUpdate,
    ) -> Result<RepositoryWriteReceipt, BackendError> {
        self.write(RepositoryWrite {
            repository: CommerceCatalogProductRecord::REPOSITORY.to_string(),
            record_id: update.handle.clone(),
            fields: BTreeMap::from([
                ("title".to_string(), update.title.clone()),
                ("summary".to_string(), update.summary.clone()),
                ("price_minor".to_string(), update.price_minor.to_string()),
                (
                    "collection_handle".to_string(),
                    update.collection_handle.clone(),
                ),
                ("is_visible".to_string(), update.is_visible.to_string()),
            ]),
        })
    }

    fn commerce_catalog_collection(
        &self,
        handle: &str,
    ) -> Result<Option<CommerceCatalogCollectionRecord>, BackendError> {
        let records = self.read(
            &RepositoryQuery::new(CommerceCatalogCollectionRecord::REPOSITORY).with_key(handle),
        )?;
        records
            .records
            .first()
            .map(CommerceCatalogCollectionRecord::from_repository_record)
            .transpose()
    }

    fn update_commerce_catalog_collection(
        &self,
        update: &CommerceCatalogCollectionUpdate,
    ) -> Result<RepositoryWriteReceipt, BackendError> {
        self.write(RepositoryWrite {
            repository: CommerceCatalogCollectionRecord::REPOSITORY.to_string(),
            record_id: update.handle.clone(),
            fields: BTreeMap::from([
                ("title".to_string(), update.title.clone()),
                ("label".to_string(), update.label.clone()),
                ("summary".to_string(), update.summary.clone()),
                ("is_visible".to_string(), update.is_visible.to_string()),
            ]),
        })
    }

    fn commerce_order(&self, order_id: &str) -> Result<Option<CommerceOrderRecord>, BackendError> {
        let records =
            self.read(&RepositoryQuery::new(CommerceOrderRecord::REPOSITORY).with_key(order_id))?;
        records
            .records
            .first()
            .map(CommerceOrderRecord::from_repository_record)
            .transpose()
    }

    fn commerce_order_by_payment_reference(
        &self,
        payment_reference: &str,
    ) -> Result<Option<CommerceOrderRecord>, BackendError> {
        let records = self.read(
            &RepositoryQuery::new(CommerceOrderRecord::REPOSITORY)
                .with_filter("payment_reference", payment_reference),
        )?;
        records
            .records
            .first()
            .map(CommerceOrderRecord::from_repository_record)
            .transpose()
    }
}

impl<T: RepositoryFacade + ?Sized> RepositoryFacadeExt for T {}

pub trait AuthFacade: Send + Sync {
    fn check_capability(&self, request: &AuthCheckRequest)
    -> Result<AuthCheckResult, BackendError>;

    fn explain_denial(&self, request: &AuthExplainRequest)
    -> Result<AuthExplanation, BackendError>;
}

pub trait AuditFacade: Send + Sync {
    fn record(&self, entry: AuditEntry) -> Result<(), BackendError>;
}

pub trait OutboundHttpFacade: Send + Sync {
    fn send(&self, request: OutboundHttpRequest) -> Result<OutboundHttpResponse, BackendError>;
}

pub trait AssetsFacade: Send + Sync {
    fn publish(&self, request: AssetWriteRequest) -> Result<AssetWriteReceipt, BackendError>;

    fn inspect(&self, logical_path: &str) -> Result<Option<ManagedAsset>, BackendError>;
}
