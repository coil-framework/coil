use crate::{
    AuditFacade, AuthFacade, BackendError, CmsPageDraft, CmsPublishDecision, CommerceFacade,
    JobsFacade, OrderDraft, OrderReviewDecision, OutboundHttpFacade, RepositoryFacade,
    RequestContext, VerifiedWebhook, WebhookHandlingResult,
};

pub trait CheckoutHooks: Send + Sync {
    fn review_order(
        &self,
        ctx: &RequestContext,
        order: &OrderDraft,
        commerce: &dyn CommerceFacade,
        auth: &dyn AuthFacade,
        audit: &dyn AuditFacade,
    ) -> Result<OrderReviewDecision, BackendError>;
}

pub trait CmsHooks: Send + Sync {
    fn validate_page_publish(
        &self,
        ctx: &RequestContext,
        draft: &CmsPageDraft,
        repositories: &dyn RepositoryFacade,
        audit: &dyn AuditFacade,
    ) -> Result<CmsPublishDecision, BackendError>;
}

pub trait VerifiedWebhookHooks: Send + Sync {
    fn handle_verified_webhook(
        &self,
        ctx: &RequestContext,
        webhook: &VerifiedWebhook,
        http: &dyn OutboundHttpFacade,
        jobs: &dyn JobsFacade,
        audit: &dyn AuditFacade,
    ) -> Result<WebhookHandlingResult, BackendError>;
}
