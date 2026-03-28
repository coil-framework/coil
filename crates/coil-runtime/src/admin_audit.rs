use super::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const ADMIN_AUDIT_LOG_FILE: &str = "admin-audit-log.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct AdminAuditEntry {
    pub recorded_at_unix_seconds: i64,
    pub actor: String,
    pub kind: String,
}

#[derive(Debug, Clone)]
pub(crate) struct AdminAuditLog {
    path: PathBuf,
}

impl AdminAuditLog {
    pub(crate) fn open(plan: &RuntimePlan) -> Self {
        let path = plan
            .shared_state_root()
            .join("admin")
            .join(ADMIN_AUDIT_LOG_FILE);
        Self { path }
    }

    pub(crate) fn location_label(&self) -> String {
        self.path.display().to_string()
    }

    pub(crate) fn load(&self) -> Result<Vec<AdminAuditEntry>, String> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let body = fs::read_to_string(&self.path).map_err(|error| {
            format!(
                "failed to read admin audit log `{}`: {error}",
                self.path.display()
            )
        })?;
        serde_json::from_str(&body).map_err(|error| {
            format!(
                "failed to parse admin audit log `{}`: {error}",
                self.path.display()
            )
        })
    }

    pub(crate) fn save(&self, entries: &[AdminAuditEntry]) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create admin audit directory `{}`: {error}",
                    parent.display()
                )
            })?;
        }
        let body = serde_json::to_string_pretty(entries).map_err(|error| {
            format!(
                "failed to serialize admin audit log `{}`: {error}",
                self.path.display()
            )
        })?;
        fs::write(&self.path, body).map_err(|error| {
            format!(
                "failed to write admin audit log `{}`: {error}",
                self.path.display()
            )
        })
    }

    pub(crate) fn record(&self, entry: AdminAuditEntry) -> Result<(), String> {
        let mut entries = self.load()?;
        entries.push(entry);
        self.save(&entries)
    }

    pub(crate) fn recent_entries(&self, limit: usize) -> Result<Vec<AdminAuditEntry>, String> {
        let mut entries = self.load()?;
        if entries.len() > limit {
            let start = entries.len() - limit;
            entries = entries.split_off(start);
        }
        Ok(entries)
    }
}

pub(crate) fn record_admin_audit_entry(
    plan: &RuntimePlan,
    recorded_at_unix_seconds: i64,
    actor: impl Into<String>,
    kind: impl Into<String>,
) -> Result<(), String> {
    AdminAuditLog::open(plan).record(AdminAuditEntry {
        recorded_at_unix_seconds,
        actor: actor.into(),
        kind: kind.into(),
    })
}
