use crate::{GitlyPage, GitlyState};
use coil::fission::core::WidgetId;
use coil::fission::prelude::*;

#[derive(Clone, Copy)]
pub struct GitlyPageView;

impl From<GitlyPageView> for Widget {
    fn from(_: GitlyPageView) -> Self {
        let (_, view) = coil::fission::build::current::<GitlyState>();
        let state = view.state();
        let env = view.env();
        let spacing = view.env().theme.tokens.spacing.xl;
        Container::new(Column {
            gap: Some(spacing),
            children: vec![header(state, env), content(state, env), footer(env)],
            ..Default::default()
        })
        .padding_all(spacing)
        .into()
    }
}

fn header(state: &GitlyState, env: &Env) -> Widget {
    Responsive::new(desktop_header(state, env))
        .case(ResponsiveCase::max_width(760.0, mobile_header(state, env)))
        .into()
}

fn desktop_header(state: &GitlyState, env: &Env) -> Widget {
    let locale = &state.locale;
    Column {
        gap: Some(16.0),
        children: vec![
            Row {
                gap: Some(22.0),
                children: vec![
                    Link::to("GITLY", local_path(locale, "/")).into(),
                    Spacer {
                        flex_grow: 1.0,
                        ..Default::default()
                    }
                    .into(),
                    Link::to(
                        t(env, "nav.explore", "Explore"),
                        local_path(locale, "/explore"),
                    )
                    .into(),
                    Link::to(
                        t(env, "nav.repository", "Repository"),
                        local_path(locale, "/forgeflow/platform-ui"),
                    )
                    .into(),
                    Link::to(
                        t(env, "nav.pulls", "Pull requests"),
                        local_path(locale, "/forgeflow/platform-ui/pulls"),
                    )
                    .into(),
                    Link::to(
                        t(env, "nav.actions", "Actions"),
                        local_path(locale, "/forgeflow/platform-ui/actions"),
                    )
                    .into(),
                    Link::to(
                        t(env, "nav.organization", "Organization"),
                        local_path(locale, "/orgs/forgeflow"),
                    )
                    .into(),
                    Link::to(
                        t(env, "nav.profile", "Profile"),
                        local_path(locale, "/alexmariner"),
                    )
                    .into(),
                ],
                ..Default::default()
            }
            .into(),
            language_switcher(state.page, env),
            Divider::default().into(),
        ],
        ..Default::default()
    }
    .into()
}

fn mobile_header(state: &GitlyState, env: &Env) -> Widget {
    let locale = &state.locale;
    Column {
        gap: Some(14.0),
        children: vec![
            Link::to("GITLY", local_path(locale, "/")).into(),
            Wrap {
                spacing: Some(14.0),
                children: vec![
                    Link::to(
                        t(env, "nav.explore", "Explore"),
                        local_path(locale, "/explore"),
                    )
                    .into(),
                    Link::to(
                        t(env, "nav.repository", "Repository"),
                        local_path(locale, "/forgeflow/platform-ui"),
                    )
                    .into(),
                    Link::to(
                        t(env, "nav.pulls", "Pulls"),
                        local_path(locale, "/forgeflow/platform-ui/pulls"),
                    )
                    .into(),
                    Link::to(
                        t(env, "nav.actions", "Actions"),
                        local_path(locale, "/forgeflow/platform-ui/actions"),
                    )
                    .into(),
                    Link::to(
                        t(env, "nav.profile", "Profile"),
                        local_path(locale, "/alexmariner"),
                    )
                    .into(),
                ],
                ..Default::default()
            }
            .into(),
            language_switcher(state.page, env),
            Divider::default().into(),
        ],
        ..Default::default()
    }
    .into()
}

fn language_switcher(page: GitlyPage, env: &Env) -> Widget {
    Row {
        gap: Some(12.0),
        children: vec![
            Text::new(t(env, "nav.language", "Language"))
                .weight(700)
                .into(),
            Link::to("EN", local_path("en-GB", page.path())).into(),
            Link::to("FR", local_path("fr-FR", page.path())).into(),
            Link::to("DE", local_path("de-DE", page.path())).into(),
        ],
        ..Default::default()
    }
    .into()
}

fn content(state: &GitlyState, env: &Env) -> Widget {
    match state.page {
        GitlyPage::Home => home(state, env),
        GitlyPage::Explore => explore(state, env),
        GitlyPage::Repository => repository(state, env),
        GitlyPage::Issues => issues(env),
        GitlyPage::PullRequests => pulls(state, env),
        GitlyPage::Actions => actions(state, env),
        GitlyPage::Organization => organization(state, env),
        GitlyPage::Profile => profile(state, env),
        GitlyPage::Search => search(state, env),
    }
}

fn home(state: &GitlyState, env: &Env) -> Widget {
    Column {
        gap: Some(28.0),
        children: vec![
            Text::new(t(env, "home.eyebrow", "A FORGE BUILT AS A CUSTOMER APP")).weight(700).into(),
            Text::new(t(env, "home.title", "Ship accessible, multilingual developer tools without hiding the extension boundaries."))
                .size(48.0)
                .weight(700)
                .into(),
            Text::new(state.repository.description.clone()).size(19.0).into(),
            Link::to(t(env, "home.action", "Explore repositories"), local_path(&state.locale, "/explore")).into(),
            Divider::default().into(),
            metric_row(state),
            Text::new(t(env, "home.truth", "Gitly is intentionally static-but-honest: the forge data is demonstrative, while the customer app, linked Rust hooks, scheduled-job contract, and bounded extension surface are real.")).into(),
        ],
        ..Default::default()
    }
    .into()
}

fn explore(state: &GitlyState, env: &Env) -> Widget {
    Column {
        gap: Some(20.0),
        children: vec![
            Text::new(t(env, "explore.title", "Explore"))
                .size(44.0)
                .weight(700)
                .into(),
            Text::new(t(
                env,
                "explore.body",
                "Customer-owned repositories and surfaces composed through Coil.",
            ))
            .into(),
            Divider::default().into(),
            Text::new(format!(
                "{}/{}",
                state.repository.owner, state.repository.name
            ))
            .size(30.0)
            .weight(700)
            .into(),
            Text::new(state.repository.description.clone()).into(),
            Link::to(
                t(env, "explore.action", "Open repository"),
                local_path(&state.locale, "/forgeflow/platform-ui"),
            )
            .into(),
        ],
        ..Default::default()
    }
    .into()
}

fn repository(state: &GitlyState, env: &Env) -> Widget {
    let repository = &state.repository;
    Column {
        gap: Some(22.0),
        children: vec![
            Text::new(repository.visibility.to_uppercase()).weight(700).into(),
            Text::new(format!("{}/{}", repository.owner, repository.name))
                .size(46.0)
                .weight(700)
                .into(),
            Text::new(repository.description.clone()).size(19.0).into(),
            repository_tabs(&state.locale, env),
            Divider::default().into(),
            Text::new(t(env, "repo.readme", "README")).size(30.0).weight(700).into(),
            Text::new(t(env, "repo.body", "Gitly demonstrates a customer-owned Fission application on Coil. Linked Rust hooks govern editorial policy, custom routes expose forge-shaped APIs, and bounded WASM packages extend selected runtime surfaces.")).into(),
            metric_row(state),
        ],
        ..Default::default()
    }
    .into()
}

fn repository_tabs(locale: &str, env: &Env) -> Widget {
    Wrap {
        spacing: Some(18.0),
        children: vec![
            Link::to(
                t(env, "repo.code", "Code"),
                local_path(locale, "/forgeflow/platform-ui"),
            )
            .into(),
            Link::to(
                t(env, "repo.issues", "Issues"),
                local_path(locale, "/forgeflow/platform-ui/issues"),
            )
            .into(),
            Link::to(
                t(env, "nav.pulls", "Pull requests"),
                local_path(locale, "/forgeflow/platform-ui/pulls"),
            )
            .into(),
            Link::to(
                t(env, "nav.actions", "Actions"),
                local_path(locale, "/forgeflow/platform-ui/actions"),
            )
            .into(),
        ],
        ..Default::default()
    }
    .into()
}

fn metric_row(state: &GitlyState) -> Widget {
    let repository = &state.repository;
    Wrap {
        spacing: Some(24.0),
        children: vec![
            Text::new(format!("{} stars", repository.stars))
                .weight(700)
                .into(),
            Text::new(format!("{} forks", repository.forks))
                .weight(700)
                .into(),
            Text::new(format!("{} watchers", repository.watchers))
                .weight(700)
                .into(),
            Text::new(format!("{} issues", repository.open_issues))
                .weight(700)
                .into(),
            Text::new(repository.primary_language.clone()).into(),
            Text::new(repository.license.clone()).into(),
        ],
        ..Default::default()
    }
    .into()
}

fn issues(env: &Env) -> Widget {
    Column {
        gap: Some(18.0),
        children: vec![
            Text::new(t(env, "repo.issues", "Issues"))
                .size(44.0)
                .weight(700)
                .into(),
            Text::new(t(
                env,
                "issues.body",
                "Triage examples for accessibility, localization, and workflow quality.",
            ))
            .into(),
            Timeline {
                items: vec![
                    TimelineItem {
                        title: "#248 Keyboard focus escapes the repository command menu".into(),
                        description: Some("Accessibility · open".into()),
                        timestamp: Some("Today".into()),
                    },
                    TimelineItem {
                        title: "#241 German navigation wraps at compact widths".into(),
                        description: Some("Localization · investigating".into()),
                        timestamp: Some("Yesterday".into()),
                    },
                    TimelineItem {
                        title: "#236 Document the scheduled extension retry contract".into(),
                        description: Some("Documentation · planned".into()),
                        timestamp: Some("This week".into()),
                    },
                ],
            }
            .into(),
        ],
        ..Default::default()
    }
    .into()
}

fn pulls(state: &GitlyState, env: &Env) -> Widget {
    Column {
        gap: Some(18.0),
        children: vec![
            Text::new(t(env, "pulls.title", "Open pull requests"))
                .size(44.0)
                .weight(700)
                .into(),
            Text::new(t(
                env,
                "pulls.body",
                "Review-shaped fixture data rendered with Fission's semantic table widget.",
            ))
            .into(),
            DataTable {
                id: WidgetId::explicit("gitly.pull-requests"),
                columns: vec![
                    table_column("pull", &t(env, "pulls.column", "Pull request"), 360.0),
                    table_column("author", &t(env, "pulls.author", "Author"), 150.0),
                    table_column("checks", &t(env, "pulls.checks", "Checks"), 150.0),
                    table_column("status", &t(env, "pulls.status", "Status"), 170.0),
                ],
                rows: state
                    .pull_requests
                    .iter()
                    .map(|pull| TableRow {
                        id: format!("pull:{}", pull.number),
                        cells: vec![
                            format!("#{} {}", pull.number, pull.title),
                            pull.author.clone(),
                            pull.checks.clone(),
                            pull.status.clone(),
                        ],
                    })
                    .collect(),
                selected_ids: Vec::new(),
                on_selection_change: None,
            }
            .into(),
        ],
        ..Default::default()
    }
    .into()
}

fn actions(state: &GitlyState, env: &Env) -> Widget {
    let mut items = state
        .workflows
        .iter()
        .map(|run| TimelineItem {
            title: format!("{} · {}", run.workflow, run.status),
            description: Some(format!(
                "{} {} {} · {}",
                run.trigger,
                t(env, "actions.on", "on"),
                run.branch,
                run.cadence
            )),
            timestamp: None,
        })
        .collect::<Vec<_>>();
    items.push(TimelineItem {
        title: state.scheduler_contract.clone(),
        description: Some(format!(
            "{} {}",
            t(env, "actions.provided_by", "Provided by"),
            state.scheduler_extension
        )),
        timestamp: Some(t(
            env,
            "actions.scheduled_contract",
            "Scheduled extension contract",
        )),
    });
    Column {
        gap: Some(18.0),
        children: vec![
            Text::new(t(env, "nav.actions", "Actions"))
                .size(44.0)
                .weight(700)
                .into(),
            Text::new(t(
                env,
                "actions.body",
                "Workflow fixtures alongside the real bounded scheduler registration.",
            ))
            .into(),
            Timeline { items }.into(),
        ],
        ..Default::default()
    }
    .into()
}

fn organization(state: &GitlyState, env: &Env) -> Widget {
    let organization = &state.organization;
    Column {
        gap: Some(18.0),
        children: vec![
            Text::new(organization.name.clone())
                .size(44.0)
                .weight(700)
                .into(),
            Text::new(format!(
                "@{} · {}",
                organization.handle, organization.location
            ))
            .into(),
            Text::new(organization.summary.clone()).size(19.0).into(),
            Divider::default().into(),
            Text::new(format!(
                "{} {} · {} {}",
                organization.members,
                t(env, "organization.members", "members"),
                organization.repositories,
                t(env, "organization.repositories", "repositories")
            ))
            .weight(700)
            .into(),
        ],
        ..Default::default()
    }
    .into()
}

fn profile(state: &GitlyState, env: &Env) -> Widget {
    let user = &state.user;
    Column {
        gap: Some(18.0),
        children: vec![
            Text::new(user.display_name.clone())
                .size(44.0)
                .weight(700)
                .into(),
            Text::new(format!(
                "@{} · {} · {}",
                user.handle, user.role, user.location
            ))
            .into(),
            Text::new(user.bio.clone()).size(19.0).into(),
            Text::new(format!(
                "{} {} · {} {}",
                user.repositories,
                t(env, "profile.repositories", "repositories"),
                user.followers,
                t(env, "profile.followers", "followers")
            ))
            .weight(700)
            .into(),
            Divider::default().into(),
            Text::new(t(env, "profile.contributions", "Recent contributions"))
                .size(28.0)
                .weight(700)
                .into(),
            Timeline {
                items: vec![
                    TimelineItem {
                        title: "Published repository navigation accessibility guidance".into(),
                        description: None,
                        timestamp: Some("Today".into()),
                    },
                    TimelineItem {
                        title: "Reviewed the French and German language switcher rollout".into(),
                        description: None,
                        timestamp: Some("This week".into()),
                    },
                    TimelineItem {
                        title: "Validated the bounded WASM API extension contract".into(),
                        description: None,
                        timestamp: Some("This month".into()),
                    },
                ],
            }
            .into(),
        ],
        ..Default::default()
    }
    .into()
}

fn search(state: &GitlyState, env: &Env) -> Widget {
    let query = if state.search_query.trim().is_empty() {
        "platform"
    } else {
        state.search_query.as_str()
    };
    Column {
        gap: Some(18.0),
        children: vec![
            Text::new(t(env, "search.title", "Search results")).size(44.0).weight(700).into(),
            Text::new(format!("{}: {query}", t(env, "search.query", "Query"))).weight(700).into(),
            Text::new(t(env, "search.body", "Search remains a bounded browser island over the checked-in demo index; it does not claim to search a real git forge.")).into(),
            SemanticsRegion {
                id: Some(WidgetId::explicit("gitly-search")),
                identifier: Some("gitly-search".into()),
                child: Some(
                    Text::new(t(
                        env,
                        "search.island_mount",
                        "Search Gitly's demonstration index",
                    ))
                    .into(),
                ),
                ..Default::default()
            }
            .into(),
        ],
        ..Default::default()
    }
    .into()
}

fn table_column(id: &str, title: &str, width: f32) -> TableColumn {
    TableColumn {
        id: id.into(),
        title: title.into(),
        width,
        sortable: true,
    }
}

fn footer(env: &Env) -> Widget {
    Column {
        gap: Some(16.0),
        children: vec![
            Divider::default().into(),
            Text::new(t(env, "footer.body", "Gitly is a customer-root Coil demonstration of CMS policy, custom APIs, linked Rust hooks, and bounded runtime-installed WASM.")).into(),
        ],
        ..Default::default()
    }
    .into()
}

fn t(env: &Env, key: &str, fallback: &str) -> String {
    env.i18n
        .get(&env.locale, key)
        .unwrap_or(fallback)
        .to_string()
}

fn local_path(locale: &str, path: &str) -> String {
    let prefix = match locale {
        "fr-FR" => "/fr",
        "de-DE" => "/de",
        _ => "",
    };
    if path == "/" {
        if prefix.is_empty() {
            "/".into()
        } else {
            prefix.into()
        }
    } else {
        format!("{prefix}{path}")
    }
}

#[cfg(test)]
mod tests {
    use super::{local_path, GitlyPage};

    #[test]
    fn localized_paths_preserve_the_selected_page() {
        assert_eq!(
            local_path("en-GB", "/forgeflow/platform-ui"),
            "/forgeflow/platform-ui"
        );
        assert_eq!(
            local_path("fr-FR", "/forgeflow/platform-ui"),
            "/fr/forgeflow/platform-ui"
        );
        assert_eq!(local_path("de-DE", "/"), "/de");
        assert_eq!(
            local_path("fr-FR", GitlyPage::PullRequests.path()),
            "/fr/forgeflow/platform-ui/pulls"
        );
    }
}
