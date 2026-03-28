#![forbid(unsafe_code)]

use davenda_customer_sdk::{
    AuditFacade, BackendError, CmsHooks, CmsPageDraft, CmsPublishDecision, CustomerBackendPlugin,
    CustomerHookRegistry, CustomerPluginDescriptor, RegisteredHookKind, RepositoryFacade,
};
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GitlyRepository {
    pub owner: String,
    pub name: String,
    pub visibility: String,
    pub stars: u32,
    pub forks: u32,
    pub watchers: u32,
    pub open_issues: u32,
    pub description: String,
    pub primary_language: String,
    pub license: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GitlyPullRequest {
    pub number: u32,
    pub title: String,
    pub author: String,
    pub status: String,
    pub checks: String,
    pub branch: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GitlyWorkflowRun {
    pub workflow: String,
    pub branch: String,
    pub trigger: String,
    pub status: String,
    pub cadence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GitlyOrganization {
    pub handle: String,
    pub name: String,
    pub members: u32,
    pub repositories: u32,
    pub location: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GitlyUser {
    pub handle: String,
    pub display_name: String,
    pub role: String,
    pub location: String,
    pub bio: String,
    pub repositories: u32,
    pub followers: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitlyLinkedPluginSummary {
    pub id: String,
    pub display_name: String,
    pub version: String,
    pub documentation_url: Option<String>,
    pub hook_kinds: Vec<RegisteredHookKind>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct GitlyBackend;

pub fn plugin() -> GitlyBackend {
    GitlyBackend
}

pub fn repository() -> GitlyRepository {
    GitlyRepository {
        owner: "forgeflow".to_string(),
        name: "platform-ui".to_string(),
        visibility: "Public".to_string(),
        stars: 4872,
        forks: 612,
        watchers: 143,
        open_issues: 27,
        description: "Accessible multilingual UI primitives and customer-app examples for Davenda."
            .to_string(),
        primary_language: "Rust".to_string(),
        license: "Apache-2.0".to_string(),
    }
}

pub fn pull_requests() -> Vec<GitlyPullRequest> {
    vec![
        GitlyPullRequest {
            number: 184,
            title: "Tighten ARIA labeling on repository navigation".to_string(),
            author: "alexmariner".to_string(),
            status: "Review required".to_string(),
            checks: "9/10 passing".to_string(),
            branch: "feat/a11y-repo-nav".to_string(),
        },
        GitlyPullRequest {
            number: 179,
            title: "Translate issue triage macros to French and German".to_string(),
            author: "pauline".to_string(),
            status: "Checks passed".to_string(),
            checks: "12/12 passing".to_string(),
            branch: "feat/i18n-triage".to_string(),
        },
        GitlyPullRequest {
            number: 171,
            title: "Refactor Actions dashboard polling for dark mode parity".to_string(),
            author: "mika".to_string(),
            status: "Draft".to_string(),
            checks: "4/12 running".to_string(),
            branch: "refactor/actions-dashboard".to_string(),
        },
    ]
}

pub fn workflow_runs() -> Vec<GitlyWorkflowRun> {
    vec![
        GitlyWorkflowRun {
            workflow: "UI regression".to_string(),
            branch: "main".to_string(),
            trigger: "schedule".to_string(),
            status: "Success".to_string(),
            cadence: "Every 30 minutes".to_string(),
        },
        GitlyWorkflowRun {
            workflow: "Localization smoke".to_string(),
            branch: "main".to_string(),
            trigger: "schedule".to_string(),
            status: "Running".to_string(),
            cadence: "Every hour".to_string(),
        },
        GitlyWorkflowRun {
            workflow: "WASM extension contract".to_string(),
            branch: "feat/community-badge".to_string(),
            trigger: "pull_request".to_string(),
            status: "Queued".to_string(),
            cadence: "On push and PR".to_string(),
        },
    ]
}

pub fn organization() -> GitlyOrganization {
    GitlyOrganization {
        handle: "forgeflow".to_string(),
        name: "Forgeflow".to_string(),
        members: 42,
        repositories: 18,
        location: "London, UK".to_string(),
        summary: "A modular product engineering team using Davenda to ship docs, dashboards, and internal forges from one customer workspace.".to_string(),
    }
}

pub fn user() -> GitlyUser {
    GitlyUser {
        handle: "alexmariner".to_string(),
        display_name: "Alex Mariner".to_string(),
        role: "Staff Engineer".to_string(),
        location: "Bristol, UK".to_string(),
        bio: "Building multilingual developer tools, documentation surfaces, and workflow automation demos in Davenda.".to_string(),
        repositories: 14,
        followers: 238,
    }
}

pub fn repo_api_payload() -> BTreeMap<String, String> {
    let repo = repository();
    BTreeMap::from([
        ("owner".to_string(), repo.owner),
        ("name".to_string(), repo.name),
        ("visibility".to_string(), repo.visibility),
        ("stars".to_string(), repo.stars.to_string()),
        ("forks".to_string(), repo.forks.to_string()),
        ("watchers".to_string(), repo.watchers.to_string()),
        ("open_issues".to_string(), repo.open_issues.to_string()),
        ("language".to_string(), repo.primary_language),
        ("license".to_string(), repo.license),
        ("description".to_string(), repo.description),
    ])
}

pub fn user_api_payload() -> BTreeMap<String, String> {
    let user = user();
    BTreeMap::from([
        ("handle".to_string(), user.handle),
        ("display_name".to_string(), user.display_name),
        ("role".to_string(), user.role),
        ("location".to_string(), user.location),
        ("bio".to_string(), user.bio),
        ("repositories".to_string(), user.repositories.to_string()),
        ("followers".to_string(), user.followers.to_string()),
    ])
}

pub fn organization_api_payload() -> BTreeMap<String, String> {
    let org = organization();
    BTreeMap::from([
        ("handle".to_string(), org.handle),
        ("name".to_string(), org.name),
        ("members".to_string(), org.members.to_string()),
        ("repositories".to_string(), org.repositories.to_string()),
        ("location".to_string(), org.location),
        ("summary".to_string(), org.summary),
    ])
}

pub fn pulls_api_payload() -> BTreeMap<String, String> {
    let pulls = pull_requests();
    let open = pulls.len();
    let checks_passing = pulls
        .iter()
        .filter(|pull| pull.status == "Checks passed")
        .count();
    BTreeMap::from([
        ("count".to_string(), open.to_string()),
        ("top_pull".to_string(), format!("#{} {}", pulls[0].number, pulls[0].title)),
        ("top_status".to_string(), pulls[0].status.clone()),
        ("checks_passing".to_string(), checks_passing.to_string()),
    ])
}

pub fn workflow_api_payload() -> BTreeMap<String, String> {
    let runs = workflow_runs();
    let running = runs.iter().filter(|run| run.status == "Running").count();
    let queued = runs.iter().filter(|run| run.status == "Queued").count();
    BTreeMap::from([
        ("workflow_count".to_string(), runs.len().to_string()),
        ("running".to_string(), running.to_string()),
        ("queued".to_string(), queued.to_string()),
        ("primary_workflow".to_string(), runs[0].workflow.clone()),
        ("primary_cadence".to_string(), runs[0].cadence.clone()),
        (
            "scheduler_contract".to_string(),
            "github.actions.refresh".to_string(),
        ),
        (
            "scheduler_extension".to_string(),
            "gitly-actions-scheduler".to_string(),
        ),
    ])
}

pub fn linked_plugin_summary() -> GitlyLinkedPluginSummary {
    let plugin = plugin();
    let descriptor = plugin.descriptor();
    GitlyLinkedPluginSummary {
        id: descriptor.id,
        display_name: descriptor.display_name,
        version: descriptor.version,
        documentation_url: descriptor.documentation_url,
        hook_kinds: vec![RegisteredHookKind::CmsPagePublish],
    }
}

impl CustomerBackendPlugin for GitlyBackend {
    fn descriptor(&self) -> CustomerPluginDescriptor {
        CustomerPluginDescriptor::new(
            "gitly-backend",
            "Gitly Linked Backend",
            env!("CARGO_PKG_VERSION"),
        )
        .with_documentation_url("apps/gitly/README.md")
    }

    fn register(&self, registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError> {
        registry.register_cms_hooks(Arc::new(*self))
    }
}

impl CmsHooks for GitlyBackend {
    fn validate_page_publish(
        &self,
        _ctx: &davenda_customer_sdk::RequestContext,
        draft: &CmsPageDraft,
        _repositories: &dyn RepositoryFacade,
        audit: &dyn AuditFacade,
    ) -> Result<CmsPublishDecision, BackendError> {
        if draft.slug.contains("readme")
            && !draft.body_html.to_ascii_lowercase().contains("accessibility")
        {
            return Ok(CmsPublishDecision::reject(
                "gitly.cms.readme.accessibility_required",
                "README-style documentation pages must mention accessibility guidance before they can be published.",
            ));
        }

        audit.record(
            davenda_customer_sdk::AuditEntry::new(
                "gitly.cms.publish.validated",
                "cms.page",
                draft.page_id.clone(),
                "allowed",
            )
            .with_detail(format!("Validated `{}` for Gitly publishing policy.", draft.slug)),
        )?;
        Ok(CmsPublishDecision::Allow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linked_backend_descriptor_is_stable() {
        let descriptor = davenda_customer_sdk::CustomerBackendPlugin::descriptor(&plugin());

        assert_eq!(descriptor.id, "gitly-backend");
        assert_eq!(descriptor.display_name, "Gitly Linked Backend");
    }

    #[test]
    fn linked_backend_summary_reports_cms_hook_registration() {
        let summary = linked_plugin_summary();

        assert_eq!(summary.id, "gitly-backend");
        assert_eq!(
            summary.hook_kinds,
            vec![RegisteredHookKind::CmsPagePublish]
        );
        assert_eq!(
            summary.documentation_url.as_deref(),
            Some("apps/gitly/README.md")
        );
    }

    #[test]
    fn repo_payload_exposes_github_style_summary_fields() {
        let payload = repo_api_payload();

        assert_eq!(payload.get("owner").map(String::as_str), Some("forgeflow"));
        assert_eq!(
            payload.get("name").map(String::as_str),
            Some("platform-ui")
        );
        assert!(payload.contains_key("stars"));
        assert!(payload.contains_key("description"));
    }
}
