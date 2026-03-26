use crate::RuntimePlan;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const CMS_ADMIN_WORKSPACE_FILE: &str = "cms-admin-workspace.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CmsAdminWorkspace {
    pub pages: Vec<CmsAdminPage>,
    pub navigation: Vec<CmsAdminNavigationItem>,
    pub redirects: Vec<CmsAdminRedirect>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CmsAdminPage {
    pub id: String,
    pub draft: CmsAdminPageRevision,
    pub live: Option<CmsAdminPageRevision>,
    #[serde(default)]
    pub published_once: bool,
    pub updated_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CmsAdminPageRevision {
    pub title: String,
    pub slug: String,
    pub summary: String,
    pub body_html: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CmsAdminNavigationItem {
    pub label: String,
    pub href: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CmsAdminRedirect {
    pub from: String,
    pub to: String,
    pub permanent: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CmsAdminPageInput {
    pub page_id: Option<String>,
    pub title: String,
    pub slug: String,
    pub summary: String,
    pub body_html: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CmsAdminPageStatus {
    DraftOnly,
    Published,
    PublishedWithDraft,
    Unpublished,
}

impl CmsAdminPage {
    pub(crate) fn status(&self) -> CmsAdminPageStatus {
        match (
            &self.live,
            self.was_ever_published(),
            self.draft_matches_live(),
        ) {
            (Some(_), _, true) => CmsAdminPageStatus::Published,
            (Some(_), _, false) => CmsAdminPageStatus::PublishedWithDraft,
            (None, true, _) => CmsAdminPageStatus::Unpublished,
            (None, false, _) => CmsAdminPageStatus::DraftOnly,
        }
    }

    pub(crate) fn status_label(&self) -> &'static str {
        match self.status() {
            CmsAdminPageStatus::DraftOnly => "Draft only",
            CmsAdminPageStatus::Published => "Published",
            CmsAdminPageStatus::PublishedWithDraft => "Published with draft",
            CmsAdminPageStatus::Unpublished => "Unpublished",
        }
    }

    pub(crate) fn live_path(&self) -> Option<String> {
        self.live
            .as_ref()
            .map(|revision| format!("/pages/{}", revision.slug))
    }

    pub(crate) fn preview_path(&self) -> String {
        format!("/admin/pages/preview?page={}", self.id)
    }

    pub(crate) fn publish(&mut self, updated_at: u64) {
        self.live = Some(self.draft.clone());
        self.published_once = true;
        self.updated_at = updated_at;
    }

    pub(crate) fn unpublish(&mut self, updated_at: u64) {
        self.live = None;
        self.updated_at = updated_at;
    }

    fn draft_matches_live(&self) -> bool {
        self.live.as_ref().is_some_and(|live| live == &self.draft)
    }

    fn was_ever_published(&self) -> bool {
        self.published_once
    }
}

impl CmsAdminWorkspace {
    pub(crate) fn load(plan: &RuntimePlan) -> Result<Self, String> {
        let path = workspace_path(plan);
        if !path.exists() {
            return Ok(default_workspace());
        }
        let bytes = fs::read(&path).map_err(|error| {
            format!(
                "failed to read CMS admin workspace `{}`: {error}",
                path.display()
            )
        })?;
        serde_json::from_slice(&bytes).map_err(|error| {
            format!(
                "failed to decode CMS admin workspace `{}`: {error}",
                path.display()
            )
        })
    }

    pub(crate) fn save(&self, plan: &RuntimePlan) -> Result<(), String> {
        let path = workspace_path(plan);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to prepare CMS admin workspace directory `{}`: {error}",
                    parent.display()
                )
            })?;
        }
        let bytes = serde_json::to_vec_pretty(self)
            .map_err(|error| format!("failed to encode CMS admin workspace: {error}"))?;
        fs::write(&path, bytes).map_err(|error| {
            format!(
                "failed to write CMS admin workspace `{}`: {error}",
                path.display()
            )
        })
    }

    pub(crate) fn selected_page(&self, page_id: Option<&str>) -> Option<&CmsAdminPage> {
        page_id
            .and_then(|page_id| self.pages.iter().find(|page| page.id == page_id))
            .or_else(|| self.pages.first())
    }

    pub(crate) fn selected_page_mut(&mut self, page_id: Option<&str>) -> Option<&mut CmsAdminPage> {
        if let Some(page_id) = page_id {
            return self.pages.iter_mut().find(|page| page.id == page_id);
        }
        self.pages.first_mut()
    }

    pub(crate) fn save_page_draft(
        &mut self,
        input: CmsAdminPageInput,
        updated_at: u64,
    ) -> Result<String, String> {
        let title = require_non_empty("page_title", input.title)?;
        let slug = validate_slug(input.slug)?;
        let summary = require_non_empty("page_summary", input.summary)?;
        let body_html = require_non_empty("page_body_html", input.body_html)?;
        let page_id = input
            .page_id
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| format!("page-{}", slug.replace('/', "-")));
        let draft = CmsAdminPageRevision {
            title,
            slug,
            summary,
            body_html,
        };

        if let Some(page) = self.pages.iter_mut().find(|page| page.id == page_id) {
            page.draft = draft;
            page.updated_at = updated_at;
            return Ok(page_id);
        }

        self.pages.push(CmsAdminPage {
            id: page_id.clone(),
            draft,
            live: None,
            published_once: false,
            updated_at,
        });
        self.pages.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(page_id)
    }

    pub(crate) fn publish_page(&mut self, page_id: &str, updated_at: u64) -> Result<(), String> {
        let page = self
            .pages
            .iter_mut()
            .find(|page| page.id == page_id)
            .ok_or_else(|| format!("CMS page `{page_id}` was not found"))?;
        page.publish(updated_at);
        Ok(())
    }

    pub(crate) fn unpublish_page(&mut self, page_id: &str, updated_at: u64) -> Result<(), String> {
        let page = self
            .pages
            .iter_mut()
            .find(|page| page.id == page_id)
            .ok_or_else(|| format!("CMS page `{page_id}` was not found"))?;
        page.unpublish(updated_at);
        Ok(())
    }

    pub(crate) fn save_navigation(
        &mut self,
        items: Vec<CmsAdminNavigationItem>,
    ) -> Result<(), String> {
        if items.is_empty() {
            return Err("Add at least one navigation item before saving.".to_string());
        }
        self.navigation = items;
        Ok(())
    }

    pub(crate) fn save_redirects(
        &mut self,
        redirects: Vec<CmsAdminRedirect>,
    ) -> Result<(), String> {
        self.redirects = redirects;
        Ok(())
    }

    pub(crate) fn live_page_by_slug(&self, slug: &str) -> Option<&CmsAdminPage> {
        self.pages
            .iter()
            .find(|page| page.live.as_ref().is_some_and(|live| live.slug == slug))
    }

    pub(crate) fn redirect_for_path(&self, path: &str) -> Option<&CmsAdminRedirect> {
        self.redirects.iter().find(|redirect| redirect.from == path)
    }
}

pub(crate) fn navigation_items_from_fields(
    fields: &crate::RequestFieldMap,
) -> Result<Vec<CmsAdminNavigationItem>, String> {
    let mut items = Vec::new();
    let mut index = 0;
    loop {
        let label_key = format!("nav_label_{index}");
        let href_key = format!("nav_href_{index}");
        let label = fields
            .get(&label_key)
            .and_then(|values| values.first())
            .cloned();
        let href = fields
            .get(&href_key)
            .and_then(|values| values.first())
            .cloned();
        if label.is_none() && href.is_none() {
            break;
        }
        if let (Some(label), Some(href)) = (label, href) {
            if !label.trim().is_empty() && !href.trim().is_empty() {
                items.push(CmsAdminNavigationItem {
                    label: require_non_empty("navigation_label", label)?,
                    href: validate_path("navigation_href", href)?,
                });
            }
        }
        index += 1;
    }
    if let (Some(label), Some(href)) = (
        fields
            .get("new_nav_label")
            .and_then(|values| values.first())
            .cloned(),
        fields
            .get("new_nav_href")
            .and_then(|values| values.first())
            .cloned(),
    ) {
        if !label.trim().is_empty() || !href.trim().is_empty() {
            items.push(CmsAdminNavigationItem {
                label: require_non_empty("new_nav_label", label)?,
                href: validate_path("new_nav_href", href)?,
            });
        }
    }
    Ok(items)
}

pub(crate) fn redirects_from_fields(
    fields: &crate::RequestFieldMap,
) -> Result<Vec<CmsAdminRedirect>, String> {
    let mut redirects = Vec::new();
    let mut index = 0;
    loop {
        let from_key = format!("redirect_from_{index}");
        let to_key = format!("redirect_to_{index}");
        let permanent_key = format!("redirect_permanent_{index}");
        let from = fields
            .get(&from_key)
            .and_then(|values| values.first())
            .cloned();
        let to = fields
            .get(&to_key)
            .and_then(|values| values.first())
            .cloned();
        if from.is_none() && to.is_none() {
            break;
        }
        if let (Some(from), Some(to)) = (from, to) {
            if !from.trim().is_empty() && !to.trim().is_empty() {
                redirects.push(CmsAdminRedirect {
                    from: validate_path(&from_key, from)?,
                    to: validate_path(&to_key, to)?,
                    permanent: fields.contains_key(&permanent_key),
                });
            }
        }
        index += 1;
    }
    if let (Some(from), Some(to)) = (
        fields
            .get("new_redirect_from")
            .and_then(|values| values.first())
            .cloned(),
        fields
            .get("new_redirect_to")
            .and_then(|values| values.first())
            .cloned(),
    ) {
        if !from.trim().is_empty() || !to.trim().is_empty() {
            redirects.push(CmsAdminRedirect {
                from: validate_path("new_redirect_from", from)?,
                to: validate_path("new_redirect_to", to)?,
                permanent: fields.contains_key("new_redirect_permanent"),
            });
        }
    }
    Ok(redirects)
}

fn workspace_path(plan: &RuntimePlan) -> PathBuf {
    plan.shared_state_root().join(CMS_ADMIN_WORKSPACE_FILE)
}

pub(crate) fn default_workspace() -> CmsAdminWorkspace {
    CmsAdminWorkspace {
        pages: vec![
            CmsAdminPage {
                id: "page-visit-harbor".to_string(),
                draft: CmsAdminPageRevision {
                    title: "Visit Harbor".to_string(),
                    slug: "visit-harbor".to_string(),
                    summary: "Store hours, pickup guidance, and the best way to plan an in-person visit.".to_string(),
                    body_html: "<p>Visit Harbor Shop for coastal goods, memberships, and event collections.</p><p>Use this sample page to verify editorial draft, preview, and publish workflows before customer migration.</p>".to_string(),
                },
                live: Some(CmsAdminPageRevision {
                    title: "Visit Harbor".to_string(),
                    slug: "visit-harbor".to_string(),
                    summary: "Store hours, pickup guidance, and the best way to plan an in-person visit.".to_string(),
                    body_html: "<p>Visit Harbor Shop for coastal goods, memberships, and event collections.</p><p>Use this sample page to verify editorial draft, preview, and publish workflows before customer migration.</p>".to_string(),
                }),
                published_once: true,
                updated_at: 1,
            },
            CmsAdminPage {
                id: "page-membership-guide".to_string(),
                draft: CmsAdminPageRevision {
                    title: "Membership Guide".to_string(),
                    slug: "membership-guide".to_string(),
                    summary: "Explains what the checked-in Harbor membership purchase unlocks for customers.".to_string(),
                    body_html: "<p>Membership purchases appear in the account area after checkout and become active when payment capture completes.</p>".to_string(),
                },
                live: None,
                published_once: false,
                updated_at: 1,
            },
        ],
        navigation: vec![
            CmsAdminNavigationItem {
                label: "Home".to_string(),
                href: "/".to_string(),
            },
            CmsAdminNavigationItem {
                label: "Shop".to_string(),
                href: "/en-GB/shop".to_string(),
            },
            CmsAdminNavigationItem {
                label: "Collections".to_string(),
                href: "/en-GB/shop/collections".to_string(),
            },
            CmsAdminNavigationItem {
                label: "Cart".to_string(),
                href: "/cart".to_string(),
            },
            CmsAdminNavigationItem {
                label: "Account".to_string(),
                href: "/account".to_string(),
            },
            CmsAdminNavigationItem {
                label: "Memberships".to_string(),
                href: "/account/memberships".to_string(),
            },
        ],
        redirects: vec![CmsAdminRedirect {
            from: "/legacy/home".to_string(),
            to: "/".to_string(),
            permanent: true,
        }],
    }
}

fn require_non_empty(field: &str, value: String) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(format!("`{field}` cannot be empty"))
    } else {
        Ok(trimmed.to_string())
    }
}

fn validate_slug(value: String) -> Result<String, String> {
    let trimmed = require_non_empty("page_slug", value)?;
    if trimmed.starts_with('/') {
        return Err("`page_slug` cannot start with `/`".to_string());
    }
    if trimmed.chars().all(|character| {
        character.is_ascii_lowercase()
            || character.is_ascii_digit()
            || matches!(character, '-' | '_')
    }) {
        Ok(trimmed)
    } else {
        Err("`page_slug` must use lowercase ASCII letters, numbers, `-`, or `_`".to_string())
    }
}

fn validate_path(field: &str, value: String) -> Result<String, String> {
    let trimmed = require_non_empty(field, value)?;
    if trimmed.starts_with('/') {
        Ok(trimmed)
    } else {
        Err(format!("`{field}` must start with `/`"))
    }
}
