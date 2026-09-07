use anyhow::{anyhow, Context, Result};
use coil::fission::server::{
    FissionServerApp, ServerHttpContext, ServerRenderContext, ServerResponse, WasmIsland,
    WebRouteMode,
};
use coil::{public_revalidation, SiteDefinition, SiteRegistry};
use coil_config::{Environment, PlatformConfig};
use gitly_fission::{GitlyPage, GitlyPageView, GitlyState};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct GitlyServerModel {
    state: GitlyState,
    workflow_api: BTreeMap<String, String>,
}

impl GitlyServerModel {
    pub fn from_workflow_api(workflow_api: BTreeMap<String, String>) -> Self {
        let state = gitly_state(&workflow_api);
        Self {
            state,
            workflow_api,
        }
    }
}

pub fn gitly_server_app(
    project_dir: impl Into<std::path::PathBuf>,
    config: &PlatformConfig,
    model: GitlyServerModel,
) -> Result<FissionServerApp> {
    let project_dir = project_dir.into();
    let sites = Arc::new(site_registry(config)?);
    let base_state = Arc::new(model.state);
    let public = || {
        WebRouteMode::Revalidated(public_revalidation(
            Duration::from_secs(300),
            ["gitly-showcase"],
        ))
    };
    let pages = [
        GitlyPage::Home,
        GitlyPage::Explore,
        GitlyPage::Repository,
        GitlyPage::Issues,
        GitlyPage::PullRequests,
        GitlyPage::Actions,
        GitlyPage::Organization,
        GitlyPage::Profile,
        GitlyPage::Search,
    ];
    let locales = [("en-GB", ""), ("fr-FR", "/fr"), ("de-DE", "/de")];

    let mut app = FissionServerApp::new("Gitly")
        .project_dir(&project_dir)
        .user_css(gitly_fission::GITLY_CSS)
        .static_dir("/theme", project_dir.join("theme"))
        .default_locale("en-GB")
        .locale_resolver(|ctx| {
            Ok(match ctx.route_path {
                "/fr" => "fr-FR".into(),
                path if path.starts_with("/fr/") => "fr-FR".into(),
                "/de" => "de-DE".into(),
                path if path.starts_with("/de/") => "de-DE".into(),
                _ => "en-GB".into(),
            })
        });
    for bundle in gitly_fission::translation_bundles() {
        app = app.translation_bundle(bundle);
    }
    for (locale, prefix) in locales {
        for page in &pages {
            let route = localized_path(prefix, page.path());
            app = app.route_widget_with_state(
                &route,
                page_title(locale, *page),
                Some(page_description(locale).to_string()),
                public(),
                GitlyPageView,
                state_loader(Arc::clone(&sites), Arc::clone(&base_state), locale, *page),
            );
            if matches!(page, GitlyPage::Search) {
                app = app.island(
                    &route,
                    WasmIsland::new(
                        "gitly-search",
                        "/fission/islands/gitly-search.wasm",
                        "gitly-search",
                    )
                    .entry("gitly_fission::gitly_search_island_boot")
                    .description("Filter Gitly's checked-in demonstration index."),
                );
            }
        }
    }

    let api_payloads = [
        ("/api/github/repository", gitly_backend::repo_api_payload()),
        ("/api/github/pulls", gitly_backend::pulls_api_payload()),
        ("/api/github/workflows", model.workflow_api),
        ("/api/github/org", gitly_backend::organization_api_payload()),
        ("/api/github/user", gitly_backend::user_api_payload()),
        (
            "/api/github/pulse",
            BTreeMap::from([
                ("status".to_string(), "ok".to_string()),
                ("surface".to_string(), "gitly-community-pulse".to_string()),
            ]),
        ),
    ];
    for (path, payload) in api_payloads {
        let sites = Arc::clone(&sites);
        app = app.http_handler("GET", path, move |ctx| {
            validate_public_request(&sites, ctx)?;
            let body =
                serde_json::to_vec(&payload).context("failed to encode Gitly API payload")?;
            Ok(ServerResponse::text(
                200,
                "application/json; charset=utf-8",
                body,
            ))
        });
    }
    Ok(app)
}

fn state_loader(
    sites: Arc<SiteRegistry>,
    base: Arc<GitlyState>,
    locale: &'static str,
    page: GitlyPage,
) -> impl for<'a> Fn(&ServerRenderContext<'a>) -> Result<GitlyState> + Send + Sync + 'static {
    move |ctx| {
        let host = required_host(&ctx.request.headers)?;
        let scope = sites
            .resolve(host, Some(locale), ctx.route_path, ctx.session.id())
            .context("Gitly request scope resolution failed")?;
        let mut state = (*base).clone();
        state.locale = scope.locale;
        state.page = page.clone();
        state.search_query = ctx.request.query.get("q").cloned().unwrap_or_default();
        Ok(state)
    }
}

fn validate_public_request(sites: &SiteRegistry, ctx: &ServerHttpContext<'_>) -> Result<()> {
    let host = required_host(&ctx.request.headers)?;
    sites
        .resolve(host, None, ctx.request.path.as_str(), ctx.session.id())
        .context("Gitly API request scope resolution failed")?;
    Ok(())
}

fn required_host(headers: &BTreeMap<String, String>) -> Result<&str> {
    headers
        .get("host")
        .map(String::as_str)
        .ok_or_else(|| anyhow!("Gitly requests require a Host header"))
}

fn localized_path(prefix: &str, path: &str) -> String {
    if path == "/" {
        if prefix.is_empty() {
            "/".to_string()
        } else {
            prefix.to_string()
        }
    } else {
        format!("{prefix}{path}")
    }
}

fn page_title(locale: &str, page: GitlyPage) -> &'static str {
    match (locale, page) {
        ("fr-FR", GitlyPage::Home) => "Gitly",
        ("fr-FR", GitlyPage::Explore) => "Explorer | Gitly",
        ("fr-FR", GitlyPage::Repository) => "forgeflow/platform-ui | Gitly",
        ("fr-FR", GitlyPage::Issues) => "Tickets | Gitly",
        ("fr-FR", GitlyPage::PullRequests) => "Demandes de fusion | Gitly",
        ("fr-FR", GitlyPage::Actions) => "Automatisations | Gitly",
        ("fr-FR", GitlyPage::Organization) => "Forgeflow | Gitly",
        ("fr-FR", GitlyPage::Profile) => "Alex Mariner | Gitly",
        ("fr-FR", GitlyPage::Search) => "Recherche | Gitly",
        ("de-DE", GitlyPage::Home) => "Gitly",
        ("de-DE", GitlyPage::Explore) => "Entdecken | Gitly",
        ("de-DE", GitlyPage::Repository) => "forgeflow/platform-ui | Gitly",
        ("de-DE", GitlyPage::Issues) => "Issues | Gitly",
        ("de-DE", GitlyPage::PullRequests) => "Pull Requests | Gitly",
        ("de-DE", GitlyPage::Actions) => "Automatisierung | Gitly",
        ("de-DE", GitlyPage::Organization) => "Forgeflow | Gitly",
        ("de-DE", GitlyPage::Profile) => "Alex Mariner | Gitly",
        ("de-DE", GitlyPage::Search) => "Suche | Gitly",
        (_, GitlyPage::Home) => "Gitly",
        (_, GitlyPage::Explore) => "Explore | Gitly",
        (_, GitlyPage::Repository) => "forgeflow/platform-ui | Gitly",
        (_, GitlyPage::Issues) => "Issues | Gitly",
        (_, GitlyPage::PullRequests) => "Pull requests | Gitly",
        (_, GitlyPage::Actions) => "Actions | Gitly",
        (_, GitlyPage::Organization) => "Forgeflow | Gitly",
        (_, GitlyPage::Profile) => "Alex Mariner | Gitly",
        (_, GitlyPage::Search) => "Search | Gitly",
    }
}

fn page_description(locale: &str) -> &'static str {
    match locale {
        "fr-FR" => "Gitly est la démonstration honnête d’une forge cliente construite sur Coil.",
        "de-DE" => "Gitly ist Coils ehrliche Demonstration einer kundeneigenen Forge.",
        _ => "Gitly is Coil's honest customer-root forge demonstration.",
    }
}

fn gitly_state(workflow: &BTreeMap<String, String>) -> GitlyState {
    let repository = gitly_backend::repository();
    let organization = gitly_backend::organization();
    let user = gitly_backend::user();
    GitlyState {
        locale: "en-GB".into(),
        page: GitlyPage::Home,
        repository: gitly_fission::GitlyRepository {
            owner: repository.owner,
            name: repository.name,
            visibility: repository.visibility,
            stars: repository.stars,
            forks: repository.forks,
            watchers: repository.watchers,
            open_issues: repository.open_issues,
            description: repository.description,
            primary_language: repository.primary_language,
            license: repository.license,
        },
        pull_requests: gitly_backend::pull_requests()
            .into_iter()
            .map(|pull| gitly_fission::GitlyPullRequest {
                number: pull.number,
                title: pull.title,
                author: pull.author,
                status: pull.status,
                checks: pull.checks,
                branch: pull.branch,
            })
            .collect(),
        workflows: gitly_backend::workflow_runs()
            .into_iter()
            .map(|run| gitly_fission::GitlyWorkflowRun {
                workflow: run.workflow,
                branch: run.branch,
                trigger: run.trigger,
                status: run.status,
                cadence: run.cadence,
            })
            .collect(),
        organization: gitly_fission::GitlyOrganization {
            handle: organization.handle,
            name: organization.name,
            members: organization.members,
            repositories: organization.repositories,
            location: organization.location,
            summary: organization.summary,
        },
        user: gitly_fission::GitlyUser {
            handle: user.handle,
            display_name: user.display_name,
            role: user.role,
            location: user.location,
            bio: user.bio,
            repositories: user.repositories,
            followers: user.followers,
        },
        search_query: String::new(),
        scheduler_contract: workflow
            .get("scheduler_contract")
            .cloned()
            .unwrap_or_else(|| "github.actions.refresh".into()),
        scheduler_extension: workflow
            .get("scheduler_extension")
            .cloned()
            .unwrap_or_else(|| "unregistered".into()),
    }
}

fn site_registry(config: &PlatformConfig) -> Result<SiteRegistry> {
    let scheme = if matches!(config.app.environment, Environment::Development) {
        "http"
    } else {
        "https"
    };
    let sites = config.sites.iter().map(|site| {
        let market = site
            .default_locale
            .rsplit_once('-')
            .map(|(_, market)| market)
            .unwrap_or(site.default_locale.as_str());
        let mut definition = SiteDefinition::new(
            &site.id,
            format!("{scheme}://{}", site.canonical_host),
            market,
            &site.default_locale,
        )
        .with_host(&site.canonical_host);
        for host in &site.hosts {
            definition = definition.with_host(host);
        }
        for locale in &site.supported_locales {
            definition = definition.with_locale(locale);
        }
        definition
    });
    SiteRegistry::new(sites).context("invalid Gitly site registry")
}

#[cfg(test)]
mod tests {
    use super::{localized_path, page_description, page_title};
    use gitly_fission::GitlyPage;

    #[test]
    fn route_prefixes_match_the_existing_public_urls() {
        assert_eq!(localized_path("", "/"), "/");
        assert_eq!(localized_path("/fr", "/"), "/fr");
        assert_eq!(localized_path("/de", "/explore"), "/de/explore");
    }

    #[test]
    fn page_titles_follow_the_route_locale() {
        assert_eq!(
            page_title("fr-FR", GitlyPage::PullRequests),
            "Demandes de fusion | Gitly"
        );
        assert_eq!(page_title("de-DE", GitlyPage::Search), "Suche | Gitly");
        assert!(page_description("fr-FR").contains("démonstration honnête"));
    }
}
