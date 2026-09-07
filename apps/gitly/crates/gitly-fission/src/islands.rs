use coil::fission::prelude::*;
use coil::fission::site::{run_browser_island, BrowserIslandApp};
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize)]
struct BridgeInput<T> {
    #[serde(default)]
    props: T,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct GitlySearchProps {
    #[serde(default)]
    pub entries: Vec<GitlySearchEntry>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GitlySearchEntry {
    pub title: String,
    pub kind: String,
    pub href: String,
}

#[derive(Clone, Debug, Default)]
struct GitlySearchState {
    query: String,
    entries: Vec<GitlySearchEntry>,
}

impl GlobalState for GitlySearchState {}

#[fission_reducer(GitlySearchChanged)]
fn search_changed(state: &mut GitlySearchState, ctx: &mut ReducerContext<GitlySearchState>) {
    if let Some(change) = ctx.input.text_change() {
        state.query = change.new_text.clone();
    }
}

#[derive(Clone, Copy)]
struct GitlySearchIsland;

impl From<GitlySearchIsland> for Widget {
    fn from(_: GitlySearchIsland) -> Self {
        let (ctx, view) = coil::fission::build::current::<GitlySearchState>();
        let changed = with_reducer!(ctx, GitlySearchChanged, search_changed);
        let query = view.state().query.trim().to_ascii_lowercase();
        let matches = view
            .state()
            .entries
            .iter()
            .filter(|entry| {
                query.is_empty()
                    || entry.title.to_ascii_lowercase().contains(&query)
                    || entry.kind.to_ascii_lowercase().contains(&query)
            })
            .collect::<Vec<_>>();
        let mut children = vec![
            TextInput {
                id: Some(WidgetId::explicit("gitly.search.input")),
                semantics_identifier: Some("gitly.search.input".into()),
                label: Some("Search repositories, people, docs, and workflows".into()),
                placeholder: Some("Try platform or workflow".into()),
                value: view.state().query.clone(),
                on_input: Some(changed),
                ..Default::default()
            }
            .into(),
            Text::new(format!("{} demonstration results", matches.len())).into(),
        ];
        children.extend(matches.into_iter().map(|entry| {
            Row {
                gap: Some(12.0),
                children: vec![
                    Text::new(entry.kind.to_uppercase()).weight(700).into(),
                    Link::to(entry.title.clone(), entry.href.clone()).into(),
                ],
                ..Default::default()
            }
            .into()
        }));
        Column {
            gap: Some(12.0),
            children,
            ..Default::default()
        }
        .into()
    }
}

pub fn gitly_search_island_boot(input: &str) -> String {
    let mut props = serde_json::from_str::<BridgeInput<GitlySearchProps>>(input)
        .unwrap_or_default()
        .props;
    if props.entries.is_empty() {
        props.entries = default_entries();
    }
    run_browser_island("gitly-search", input, || {
        BrowserIslandApp::new(
            "gitly-search",
            "gitly-search",
            GitlySearchState {
                query: String::new(),
                entries: props.entries,
            },
            GitlySearchIsland,
        )
    })
}

fn default_entries() -> Vec<GitlySearchEntry> {
    vec![
        GitlySearchEntry {
            title: "forgeflow/platform-ui".into(),
            kind: "repository".into(),
            href: "/forgeflow/platform-ui".into(),
        },
        GitlySearchEntry {
            title: "Alex Mariner".into(),
            kind: "person".into(),
            href: "/alexmariner".into(),
        },
        GitlySearchEntry {
            title: "UI regression".into(),
            kind: "workflow".into(),
            href: "/forgeflow/platform-ui/actions".into(),
        },
        GitlySearchEntry {
            title: "Repository accessibility guidance".into(),
            kind: "documentation".into(),
            href: "/forgeflow/platform-ui".into(),
        },
    ]
}
