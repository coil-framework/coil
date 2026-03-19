use super::*;
use davenda_auth::CapabilityExplanation;
use std::fmt;
use std::sync::OnceLock;

pub struct LiveAuthExplainHost {
    data: DataRuntimeServices,
    tenant_id: i64,
    database_url: Option<String>,
    auth_package: davenda_auth::AuthModelPackageSelection,
    explainer: OnceLock<Result<PostgresAuthExplainer, String>>,
}

impl LiveAuthExplainHost {
    pub fn from_config(
        config: &PlatformConfig,
        auth_package: davenda_auth::AuthModelPackageSelection,
    ) -> Result<Self, RuntimeAuthError> {
        if !config.auth.explain_api {
            return Err(RuntimeAuthError::ExplainApiDisabled);
        }

        let data = DataRuntimeServices::from_config(&config.database).map_err(|error| {
            RuntimeAuthError::BackendInitialization {
                reason: error.to_string(),
            }
        })?;

        Ok(Self::new(data, config.auth.tenant_id, None, auth_package))
    }

    pub fn new(
        data: DataRuntimeServices,
        tenant_id: i64,
        database_url: Option<String>,
        auth_package: davenda_auth::AuthModelPackageSelection,
    ) -> Self {
        Self {
            data,
            tenant_id,
            database_url,
            auth_package,
            explainer: OnceLock::new(),
        }
    }

    fn explainer(&self) -> Result<&PostgresAuthExplainer, RuntimeAuthError> {
        match self.explainer.get_or_init(|| self.build_explainer()) {
            Ok(explainer) => Ok(explainer),
            Err(reason) => Err(RuntimeAuthError::BackendInitialization {
                reason: reason.clone(),
            }),
        }
    }

    fn build_explainer(&self) -> Result<PostgresAuthExplainer, String> {
        let runtime = self
            .database_url
            .as_ref()
            .map(|url| self.data.with_resolved_connection_url(url.clone()))
            .unwrap_or_else(|| self.data.clone());
        let client = runtime
            .connect_lazy_postgres()
            .map_err(|error| error.to_string())?;
        let engine = zanzibar::postgres::PostgresRebacEngine::new(client.pool.clone());

        Ok(PostgresAuthExplainer {
            auth: davenda_auth::DavendaAuth::new(engine, self.tenant_id),
            package: self.auth_package.clone(),
        })
    }

    pub async fn explain_capability(
        &self,
        request: &LiveAuthExplainRequest,
    ) -> Result<CapabilityExplanation, RuntimeAuthError> {
        self.explainer()?.explain_capability(request).await
    }
}

impl fmt::Debug for LiveAuthExplainHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LiveAuthExplainHost")
            .field("tenant_id", &self.tenant_id)
            .field("auth_package", &self.auth_package.manifest().name)
            .finish_non_exhaustive()
    }
}

struct PostgresAuthExplainer {
    auth: davenda_auth::DavendaAuth<zanzibar::postgres::PostgresRebacEngine>,
    package: davenda_auth::AuthModelPackageSelection,
}

impl fmt::Debug for PostgresAuthExplainer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PostgresAuthExplainer")
            .field("auth_package", &self.package.manifest().name)
            .finish_non_exhaustive()
    }
}

impl PostgresAuthExplainer {
    async fn explain_capability(
        &self,
        request: &LiveAuthExplainRequest,
    ) -> Result<CapabilityExplanation, RuntimeAuthError> {
        self.auth
            .explain_capability_with_options(
                self.package.package(),
                &request.subject,
                request.capability,
                &request.object,
                request.options,
            )
            .await
            .map_err(|error| RuntimeAuthError::Explain {
                reason: error.to_string(),
            })
    }
}
