use coil::fission::prelude::GlobalState;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitlyPullRequest {
    pub number: u32,
    pub title: String,
    pub author: String,
    pub status: String,
    pub checks: String,
    pub branch: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitlyWorkflowRun {
    pub workflow: String,
    pub branch: String,
    pub trigger: String,
    pub status: String,
    pub cadence: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitlyOrganization {
    pub handle: String,
    pub name: String,
    pub members: u32,
    pub repositories: u32,
    pub location: String,
    pub summary: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitlyUser {
    pub handle: String,
    pub display_name: String,
    pub role: String,
    pub location: String,
    pub bio: String,
    pub repositories: u32,
    pub followers: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GitlyPage {
    Home,
    Explore,
    Repository,
    Issues,
    PullRequests,
    Actions,
    Organization,
    Profile,
    Search,
}

impl GitlyPage {
    pub const fn path(self) -> &'static str {
        match self {
            Self::Home => "/",
            Self::Explore => "/explore",
            Self::Repository => "/forgeflow/platform-ui",
            Self::Issues => "/forgeflow/platform-ui/issues",
            Self::PullRequests => "/forgeflow/platform-ui/pulls",
            Self::Actions => "/forgeflow/platform-ui/actions",
            Self::Organization => "/orgs/forgeflow",
            Self::Profile => "/alexmariner",
            Self::Search => "/search",
        }
    }
}

#[derive(Clone, Debug)]
pub struct GitlyState {
    pub locale: String,
    pub page: GitlyPage,
    pub repository: GitlyRepository,
    pub pull_requests: Vec<GitlyPullRequest>,
    pub workflows: Vec<GitlyWorkflowRun>,
    pub organization: GitlyOrganization,
    pub user: GitlyUser,
    pub search_query: String,
    pub scheduler_contract: String,
    pub scheduler_extension: String,
}

impl GlobalState for GitlyState {}
