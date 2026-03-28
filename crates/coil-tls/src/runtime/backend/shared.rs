use std::future::Future;

use super::super::planning::{ChallengeTicket, HotReloadEvent, RenewalPlan, TlsRuntime};
use super::super::state::TlsControlPlaneState;
use super::TlsControlPlaneStore;
use crate::{CertificateId, CertificateRecord, CertificateStatus, TlsInstant, TlsModelError};
use coil_data::{DataRuntime, PostgresDataClient};
use sqlx::{Postgres, Row};
use tokio::runtime::Runtime;

#[derive(Debug)]
pub struct PostgresTlsControlPlaneStore {
    client: PostgresDataClient,
    namespace: String,
    schema: String,
    state_table: String,
    runtime: Runtime,
}

impl PostgresTlsControlPlaneStore {
    pub fn new(
        data_runtime: &DataRuntime,
        namespace: impl Into<String>,
    ) -> Result<Self, TlsModelError> {
        let namespace = namespace.into();
        let client = data_runtime.connect_lazy_postgres().map_err(|error| {
            TlsModelError::DistributedControlPlaneStatePersistence {
                namespace: namespace.clone(),
                reason: error.to_string(),
            }
        })?;
        let runtime = Runtime::new().map_err(|error| {
            TlsModelError::DistributedControlPlaneStatePersistence {
                namespace: namespace.clone(),
                reason: error.to_string(),
            }
        })?;

        Ok(Self {
            schema: data_runtime.schema.clone(),
            state_table: state_table_name(&data_runtime.schema),
            client,
            namespace,
            runtime,
        })
    }

    fn block_on<T>(&self, future: impl Future<Output = T>) -> T {
        self.runtime.block_on(future)
    }

    async fn ensure_schema(
        pool: sqlx::Pool<Postgres>,
        namespace: String,
        schema: String,
        state_table: String,
    ) -> Result<(), TlsModelError> {
        sqlx::query(&format!(
            "CREATE SCHEMA IF NOT EXISTS {}",
            quote_identifier(&schema)
        ))
        .execute(&pool)
        .await
        .map_err(
            |error| TlsModelError::DistributedControlPlaneStatePersistence {
                namespace: namespace.clone(),
                reason: error.to_string(),
            },
        )?;
        sqlx::query(&format!(
            "CREATE TABLE IF NOT EXISTS {state_table} (
                namespace TEXT PRIMARY KEY,
                payload TEXT NOT NULL
            )"
        ))
        .execute(&pool)
        .await
        .map_err(
            |error| TlsModelError::DistributedControlPlaneStatePersistence {
                namespace,
                reason: error.to_string(),
            },
        )?;
        Ok(())
    }

    async fn read_state(
        pool: sqlx::Pool<Postgres>,
        namespace: String,
        schema: String,
        state_table: String,
    ) -> Result<TlsControlPlaneState, TlsModelError> {
        Self::ensure_schema(
            pool.clone(),
            namespace.clone(),
            schema.clone(),
            state_table.clone(),
        )
        .await?;

        let payload = sqlx::query(&format!(
            "SELECT payload FROM {state_table} WHERE namespace = $1"
        ))
        .bind(&namespace)
        .fetch_optional(&pool)
        .await
        .map_err(
            |error| TlsModelError::DistributedControlPlaneStatePersistence {
                namespace: namespace.clone(),
                reason: error.to_string(),
            },
        )?
        .map(|row| row.get::<String, _>("payload"));

        match payload {
            Some(payload) => serde_json::from_str(&payload).map_err(|error| {
                TlsModelError::CorruptDistributedControlPlaneState {
                    namespace,
                    reason: error.to_string(),
                }
            }),
            None => Ok(TlsControlPlaneState::default()),
        }
    }

    async fn mutate_state<T>(
        pool: sqlx::Pool<Postgres>,
        namespace: String,
        schema: String,
        state_table: String,
        op: impl FnOnce(&mut TlsControlPlaneState) -> Result<T, TlsModelError>,
    ) -> Result<T, TlsModelError> {
        Self::ensure_schema(
            pool.clone(),
            namespace.clone(),
            schema.clone(),
            state_table.clone(),
        )
        .await?;

        let mut tx = pool.begin().await.map_err(|error| {
            TlsModelError::DistributedControlPlaneStatePersistence {
                namespace: namespace.clone(),
                reason: error.to_string(),
            }
        })?;

        let default_payload =
            serde_json::to_string(&TlsControlPlaneState::default()).map_err(|error| {
                TlsModelError::DistributedControlPlaneStatePersistence {
                    namespace: namespace.clone(),
                    reason: error.to_string(),
                }
            })?;

        sqlx::query(&format!(
            "INSERT INTO {state_table} (namespace, payload) VALUES ($1, $2)
             ON CONFLICT (namespace) DO NOTHING"
        ))
        .bind(&namespace)
        .bind(default_payload)
        .execute(&mut *tx)
        .await
        .map_err(
            |error| TlsModelError::DistributedControlPlaneStatePersistence {
                namespace: namespace.clone(),
                reason: error.to_string(),
            },
        )?;

        let payload = sqlx::query(&format!(
            "SELECT payload FROM {state_table} WHERE namespace = $1 FOR UPDATE"
        ))
        .bind(&namespace)
        .fetch_one(&mut *tx)
        .await
        .map_err(
            |error| TlsModelError::DistributedControlPlaneStatePersistence {
                namespace: namespace.clone(),
                reason: error.to_string(),
            },
        )?
        .get::<String, _>("payload");

        let mut state = serde_json::from_str(&payload).map_err(|error| {
            TlsModelError::CorruptDistributedControlPlaneState {
                namespace: namespace.clone(),
                reason: error.to_string(),
            }
        })?;

        let outcome = op(&mut state)?;
        let serialized = serde_json::to_string(&state).map_err(|error| {
            TlsModelError::DistributedControlPlaneStatePersistence {
                namespace: namespace.clone(),
                reason: error.to_string(),
            }
        })?;

        sqlx::query(&format!(
            "UPDATE {state_table} SET payload = $2 WHERE namespace = $1"
        ))
        .bind(&namespace)
        .bind(serialized)
        .execute(&mut *tx)
        .await
        .map_err(
            |error| TlsModelError::DistributedControlPlaneStatePersistence {
                namespace: namespace.clone(),
                reason: error.to_string(),
            },
        )?;

        tx.commit().await.map_err(|error| {
            TlsModelError::DistributedControlPlaneStatePersistence {
                namespace,
                reason: error.to_string(),
            }
        })?;
        Ok(outcome)
    }
}

impl TlsControlPlaneStore for PostgresTlsControlPlaneStore {
    fn snapshot(&self) -> TlsControlPlaneState {
        self.block_on(Self::read_state(
            self.client.pool.clone(),
            self.namespace.clone(),
            self.schema.clone(),
            self.state_table.clone(),
        ))
        .expect("distributed TLS control-plane state should be readable")
    }

    fn import_certificate(&self, record: CertificateRecord) -> Result<(), TlsModelError> {
        self.block_on(Self::mutate_state(
            self.client.pool.clone(),
            self.namespace.clone(),
            self.schema.clone(),
            self.state_table.clone(),
            |state| state.inventory.insert(record),
        ))
        .map(|_| ())
    }

    fn queue_renewal(
        &self,
        runtime: &TlsRuntime,
        certificate_id: &CertificateId,
        now: TlsInstant,
    ) -> Result<RenewalPlan, TlsModelError> {
        self.block_on(Self::mutate_state(
            self.client.pool.clone(),
            self.namespace.clone(),
            self.schema.clone(),
            self.state_table.clone(),
            |state| {
                let record = state
                    .inventory
                    .record(certificate_id)
                    .cloned()
                    .ok_or_else(|| TlsModelError::UnknownCertificate {
                        certificate_id: certificate_id.to_string(),
                    })?;
                let plan = runtime.planner().renewal_plan(&record, now)?;
                if plan.renew_after > now {
                    return Err(TlsModelError::RenewalNotDue {
                        certificate_id: certificate_id.to_string(),
                        renew_after: plan.renew_after,
                        now,
                    });
                }

                if let Some(existing) = state
                    .renewal_queue
                    .iter()
                    .find(|plan| plan.certificate_id == *certificate_id)
                {
                    return Err(TlsModelError::RenewalAlreadyInProgress {
                        certificate_id: existing.certificate_id.to_string(),
                    });
                }

                if let Some(record) = state.inventory.record_mut(certificate_id) {
                    record.status = CertificateStatus::RenewalDue;
                }
                state.renewal_queue.push(plan.clone());
                Ok(plan)
            },
        ))
    }

    fn begin_renewal(
        &self,
        runtime: &TlsRuntime,
        certificate_id: &CertificateId,
        replacement_certificate_id: CertificateId,
    ) -> Result<ChallengeTicket, TlsModelError> {
        self.block_on(Self::mutate_state(
            self.client.pool.clone(),
            self.namespace.clone(),
            self.schema.clone(),
            self.state_table.clone(),
            |state| {
                let record = state.inventory.record_mut(certificate_id).ok_or_else(|| {
                    TlsModelError::UnknownCertificate {
                        certificate_id: certificate_id.to_string(),
                    }
                })?;
                if record.replacing_certificate.is_some() {
                    return Err(TlsModelError::RenewalAlreadyInProgress {
                        certificate_id: certificate_id.to_string(),
                    });
                }

                record.status = CertificateStatus::Renewing;
                record.replacing_certificate = Some(replacement_certificate_id.clone());

                let ticket = ChallengeTicket {
                    certificate_id: certificate_id.clone(),
                    replacement_certificate_id: Some(replacement_certificate_id),
                    provider: record.provider,
                    challenge: runtime.challenge,
                    bindings: record.bindings.clone(),
                    account_secret_ref: runtime.account_secret_ref.clone(),
                };
                state.pending_challenges.push(ticket.clone());
                Ok(ticket)
            },
        ))
    }

    fn fail_renewal(
        &self,
        certificate_id: &CertificateId,
    ) -> Result<CertificateRecord, TlsModelError> {
        self.block_on(Self::mutate_state(
            self.client.pool.clone(),
            self.namespace.clone(),
            self.schema.clone(),
            self.state_table.clone(),
            |state| {
                let record = {
                    let record = state.inventory.record_mut(certificate_id).ok_or_else(|| {
                        TlsModelError::UnknownCertificate {
                            certificate_id: certificate_id.to_string(),
                        }
                    })?;
                    record.status = CertificateStatus::RenewalDue;
                    record.replacing_certificate = None;
                    record.clone()
                };

                state
                    .pending_challenges
                    .retain(|ticket| &ticket.certificate_id != certificate_id);
                state
                    .renewal_queue
                    .retain(|plan| &plan.certificate_id != certificate_id);
                Ok(record)
            },
        ))
    }

    fn activate_replacement(
        &self,
        runtime: &TlsRuntime,
        certificate_id: &CertificateId,
        mut replacement: CertificateRecord,
    ) -> Result<HotReloadEvent, TlsModelError> {
        self.block_on(Self::mutate_state(
            self.client.pool.clone(),
            self.namespace.clone(),
            self.schema.clone(),
            self.state_table.clone(),
            |state| {
                replacement.status = CertificateStatus::Active;
                replacement.replacing_certificate = None;

                state
                    .inventory
                    .activate_replacement(certificate_id, replacement.clone())?;
                state
                    .pending_challenges
                    .retain(|ticket| &ticket.certificate_id != certificate_id);
                state
                    .renewal_queue
                    .retain(|plan| &plan.certificate_id != certificate_id);

                let event = HotReloadEvent {
                    certificate_id: replacement.id.clone(),
                    bindings: replacement.bindings.clone(),
                    reloaded_without_restart: runtime.hot_reload_supported,
                };
                state.hot_reload_events.push(event.clone());
                Ok(event)
            },
        ))
    }
}

fn state_table_name(schema: &str) -> String {
    format!(
        "{}.{}",
        quote_identifier(schema),
        quote_identifier("tls_control_plane_state")
    )
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}
