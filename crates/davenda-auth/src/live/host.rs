use super::*;
use std::fmt;
use std::sync::OnceLock;

pub struct LiveAuthExplainHost {
    data: DataRuntime,
    tenant_id: i64,
    database_url: Option<String>,
    auth_package: AuthModelPackageSelection,
    explainer: OnceLock<Result<PostgresAuthExplainer, String>>,
}

impl fmt::Debug for LiveAuthExplainHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LiveAuthExplainHost")
            .field("tenant_id", &self.tenant_id)
            .field("auth_package", &self.auth_package.manifest().name)
            .finish_non_exhaustive()
    }
}

impl LiveAuthExplainHost {
    pub fn from_config(
        config: &PlatformConfig,
        auth_package: AuthModelPackageSelection,
    ) -> Result<Self, LiveAuthError> {
        if !config.auth.explain_api {
            return Err(LiveAuthError::ExplainApiDisabled);
        }

        let data = DataRuntime::from_config(&config.database).map_err(|error| {
            LiveAuthError::BackendInitialization {
                reason: error.to_string(),
            }
        })?;

        Ok(Self::new(data, config.auth.tenant_id, None, auth_package))
    }

    pub fn new(
        data: DataRuntime,
        tenant_id: i64,
        database_url: Option<String>,
        auth_package: AuthModelPackageSelection,
    ) -> Self {
        Self {
            data,
            tenant_id,
            database_url,
            auth_package,
            explainer: OnceLock::new(),
        }
    }

    fn explainer(&self) -> Result<&PostgresAuthExplainer, LiveAuthError> {
        match self.explainer.get_or_init(|| self.build_explainer()) {
            Ok(explainer) => Ok(explainer),
            Err(reason) => Err(LiveAuthError::BackendInitialization {
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
            auth: DavendaAuth::new(engine, self.tenant_id),
            package: self.auth_package.clone(),
        })
    }

    pub async fn explain_capability(
        &self,
        request: &LiveAuthExplainRequest,
    ) -> Result<CapabilityExplanation, LiveAuthError> {
        self.explainer()?.explain_capability(request).await
    }
}

#[derive(Clone)]
struct PostgresAuthExplainer {
    auth: DavendaAuth<zanzibar::postgres::PostgresRebacEngine>,
    package: AuthModelPackageSelection,
}

impl PostgresAuthExplainer {
    async fn explain_capability(
        &self,
        request: &LiveAuthExplainRequest,
    ) -> Result<CapabilityExplanation, LiveAuthError> {
        self.auth
            .explain_capability_with_options(
                self.package.package(),
                &request.subject,
                request.capability,
                &request.object,
                request.options,
            )
            .await
            .map_err(|error| LiveAuthError::Explain {
                reason: error.to_string(),
            })
    }
}
