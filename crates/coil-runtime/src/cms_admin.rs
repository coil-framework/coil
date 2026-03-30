use crate::RuntimePlan;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::PathBuf;

const CMS_ADMIN_WORKSPACE_FILE: &str = "cms-admin-workspace.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CmsAdminWorkspace {
    pub pages: Vec<CmsAdminPage>,
    pub navigation: Vec<CmsAdminNavigationItem>,
    pub redirects: Vec<CmsAdminRedirect>,
    #[serde(default)]
    pub shared_blocks: Vec<CmsAdminSharedBlock>,
    #[serde(default = "default_global_settings")]
    pub global_settings: CmsAdminGlobalSettings,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CmsAdminPage {
    pub id: String,
    pub draft: CmsAdminPageRevision,
    pub live: Option<CmsAdminPageRevision>,
    #[serde(default)]
    pub previous_live: Option<CmsAdminPageRevision>,
    #[serde(default)]
    pub scheduled_publish_at: Option<u64>,
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
    #[serde(default = "default_page_settings")]
    pub settings: CmsAdminPageSettings,
    #[serde(default)]
    pub blocks: Vec<CmsAdminPageBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CmsAdminPageSettings {
    pub page_type: String,
    pub template: Option<String>,
    pub seo_title: Option<String>,
    pub seo_description: Option<String>,
    #[serde(default)]
    pub options: CmsAdminPageOptions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) struct CmsAdminPageOptions {
    pub show_in_navigation: bool,
    pub allow_indexing: bool,
    pub localized: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum CmsAdminPageBlock {
    Instance(CmsAdminBlockInstance),
    SharedReference(CmsAdminSharedBlockReference),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CmsAdminBlockInstance {
    pub id: String,
    pub block_type: String,
    pub label: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub fields: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CmsAdminSharedBlockReference {
    pub id: String,
    pub shared_block_id: String,
    pub label: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CmsAdminSharedBlock {
    pub id: String,
    pub label: String,
    pub block_type: String,
    #[serde(default)]
    pub fields: BTreeMap<String, String>,
    pub updated_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CmsAdminGlobalSettings {
    pub footer_heading: String,
    pub footer_body: String,
    pub contact_email: String,
    pub contact_phone: String,
    pub announcement_title: String,
    pub announcement_body: String,
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
    Scheduled,
    Published,
    PublishedWithDraft,
    PublishedWithScheduledDraft,
    Unpublished,
}

impl fmt::Display for CmsAdminPageStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DraftOnly => f.write_str("draft_only"),
            Self::Scheduled => f.write_str("scheduled"),
            Self::Published => f.write_str("published"),
            Self::PublishedWithDraft => f.write_str("published_with_draft"),
            Self::PublishedWithScheduledDraft => f.write_str("published_with_scheduled_draft"),
            Self::Unpublished => f.write_str("unpublished"),
        }
    }
}

impl CmsAdminPage {
    pub(crate) fn status(&self) -> CmsAdminPageStatus {
        match (
            &self.live,
            self.scheduled_publish_at,
            self.was_ever_published(),
            self.draft_matches_live(),
        ) {
            (Some(_), Some(_), _, _) => CmsAdminPageStatus::PublishedWithScheduledDraft,
            (None, Some(_), _, _) => CmsAdminPageStatus::Scheduled,
            (Some(_), None, _, true) => CmsAdminPageStatus::Published,
            (Some(_), None, _, false) => CmsAdminPageStatus::PublishedWithDraft,
            (None, None, true, _) => CmsAdminPageStatus::Unpublished,
            (None, None, false, _) => CmsAdminPageStatus::DraftOnly,
        }
    }

    pub(crate) fn status_label(&self) -> &'static str {
        match self.status() {
            CmsAdminPageStatus::DraftOnly => "Draft only",
            CmsAdminPageStatus::Scheduled => "Scheduled",
            CmsAdminPageStatus::Published => "Published",
            CmsAdminPageStatus::PublishedWithDraft => "Published with draft",
            CmsAdminPageStatus::PublishedWithScheduledDraft => "Published with scheduled draft",
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
        if let Some(current_live) = self.live.clone() {
            self.previous_live = Some(current_live);
        }
        self.live = Some(self.draft.clone());
        self.scheduled_publish_at = None;
        self.published_once = true;
        self.updated_at = updated_at;
    }

    pub(crate) fn schedule(
        &mut self,
        publish_at: u64,
        now: u64,
        updated_at: u64,
    ) -> Result<(), String> {
        if publish_at <= now {
            return Err(format!(
                "scheduled publish time `{publish_at}` must be greater than current time `{now}`"
            ));
        }
        self.scheduled_publish_at = Some(publish_at);
        self.updated_at = updated_at;
        Ok(())
    }

    pub(crate) fn apply_scheduled_publish(&mut self, now: u64) -> bool {
        if self
            .scheduled_publish_at
            .is_some_and(|publish_at| publish_at <= now)
        {
            self.publish(now);
            return true;
        }
        false
    }

    pub(crate) fn unpublish(&mut self, updated_at: u64) {
        self.live = None;
        self.scheduled_publish_at = None;
        self.updated_at = updated_at;
    }

    pub(crate) fn rollback(&mut self, updated_at: u64) -> Result<(), String> {
        let previous_live = self
            .previous_live
            .clone()
            .ok_or_else(|| "no previous live revision is available for rollback".to_string())?;
        let current_live = self.live.clone();
        self.draft = previous_live.clone();
        self.live = Some(previous_live);
        self.previous_live = current_live;
        self.scheduled_publish_at = None;
        self.published_once = true;
        self.updated_at = updated_at;
        Ok(())
    }

    pub(crate) fn has_rollback_target(&self) -> bool {
        self.previous_live.is_some()
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
        let blocks = default_blocks_from_body_html(&body_html);
        let draft = CmsAdminPageRevision {
            title,
            slug,
            summary,
            body_html,
            settings: default_page_settings(),
            blocks,
        };

        if let Some(page) = self.pages.iter_mut().find(|page| page.id == page_id) {
            let legacy_compatible_blocks =
                page.draft.blocks == default_blocks_from_body_html(&page.draft.body_html);
            page.draft = CmsAdminPageRevision {
                settings: page.draft.settings.clone(),
                blocks: if legacy_compatible_blocks {
                    draft.blocks.clone()
                } else {
                    page.draft.blocks.clone()
                },
                ..draft
            };
            page.updated_at = updated_at;
            return Ok(page_id);
        }

        self.pages.push(CmsAdminPage {
            id: page_id.clone(),
            draft,
            live: None,
            previous_live: None,
            scheduled_publish_at: None,
            published_once: false,
            updated_at,
        });
        self.pages.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(page_id)
    }

    pub(crate) fn save_page_settings(
        &mut self,
        page_id: &str,
        settings: CmsAdminPageSettings,
        updated_at: u64,
    ) -> Result<(), String> {
        let page = self
            .pages
            .iter_mut()
            .find(|page| page.id == page_id)
            .ok_or_else(|| format!("CMS page `{page_id}` was not found"))?;
        page.draft.settings = validate_page_settings(settings)?;
        page.updated_at = updated_at;
        Ok(())
    }

    pub(crate) fn replace_page_blocks(
        &mut self,
        page_id: &str,
        blocks: Vec<CmsAdminPageBlock>,
        updated_at: u64,
    ) -> Result<(), String> {
        validate_page_blocks(&blocks, &self.shared_blocks)?;
        let page = self
            .pages
            .iter_mut()
            .find(|page| page.id == page_id)
            .ok_or_else(|| format!("CMS page `{page_id}` was not found"))?;
        page.draft.blocks = blocks;
        page.updated_at = updated_at;
        Ok(())
    }

    pub(crate) fn duplicate_page_block(
        &mut self,
        page_id: &str,
        block_id: &str,
        updated_at: u64,
    ) -> Result<String, String> {
        let page = self
            .pages
            .iter_mut()
            .find(|page| page.id == page_id)
            .ok_or_else(|| format!("CMS page `{page_id}` was not found"))?;
        let source_index = page
            .draft
            .blocks
            .iter()
            .position(|block| block.id() == block_id)
            .ok_or_else(|| format!("CMS page block `{block_id}` was not found"))?;
        let mut duplicated = page.draft.blocks[source_index].clone();
        let duplicated_id = next_duplicate_block_id(&page.draft.blocks, block_id);
        duplicated.set_id(duplicated_id.clone());
        page.draft.blocks.insert(source_index + 1, duplicated);
        page.updated_at = updated_at;
        Ok(duplicated_id)
    }

    pub(crate) fn save_shared_block(
        &mut self,
        block: CmsAdminSharedBlock,
    ) -> Result<String, String> {
        let block = validate_shared_block(block)?;
        let block_id = block.id.clone();
        if let Some(existing) = self
            .shared_blocks
            .iter_mut()
            .find(|existing| existing.id == block_id)
        {
            *existing = block;
        } else {
            self.shared_blocks.push(block);
            self.shared_blocks
                .sort_by(|left, right| left.id.cmp(&right.id));
        }
        Ok(block_id)
    }

    pub(crate) fn save_global_settings(
        &mut self,
        settings: CmsAdminGlobalSettings,
    ) -> Result<(), String> {
        self.global_settings = validate_global_settings(settings)?;
        Ok(())
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

    pub(crate) fn schedule_page_publish(
        &mut self,
        page_id: &str,
        publish_at: u64,
        now: u64,
        updated_at: u64,
    ) -> Result<(), String> {
        let page = self
            .pages
            .iter_mut()
            .find(|page| page.id == page_id)
            .ok_or_else(|| format!("CMS page `{page_id}` was not found"))?;
        page.schedule(publish_at, now, updated_at)
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

    pub(crate) fn rollback_page(&mut self, page_id: &str, updated_at: u64) -> Result<(), String> {
        let page = self
            .pages
            .iter_mut()
            .find(|page| page.id == page_id)
            .ok_or_else(|| format!("CMS page `{page_id}` was not found"))?;
        page.rollback(updated_at)
    }

    pub(crate) fn apply_due_schedules(&mut self, now: u64) -> Vec<String> {
        let mut applied = Vec::new();
        for page in &mut self.pages {
            if page.apply_scheduled_publish(now) {
                applied.push(page.id.clone());
            }
        }
        applied
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
                    body_html: "<p>Visit Shoppr for coastal goods, memberships, and event collections.</p><p>Use this sample page to verify editorial draft, preview, and publish workflows before customer migration.</p>".to_string(),
                    settings: CmsAdminPageSettings {
                        page_type: "landing_page".to_string(),
                        template: Some("pages/landing-page".to_string()),
                        seo_title: Some("Visit Shoppr".to_string()),
                        seo_description: Some("Store hours and planning guidance for in-person visits.".to_string()),
                        options: CmsAdminPageOptions {
                            show_in_navigation: true,
                            allow_indexing: true,
                            localized: false,
                        },
                    },
                    blocks: vec![
                        CmsAdminPageBlock::Instance(CmsAdminBlockInstance {
                            id: "block-visit-hero".to_string(),
                            block_type: "hero".to_string(),
                            label: Some("Visit hero".to_string()),
                            enabled: true,
                            fields: BTreeMap::from([
                                ("heading".to_string(), "Visit Shoppr".to_string()),
                                (
                                    "body".to_string(),
                                    "Store hours, pickup guidance, and the best time to visit."
                                        .to_string(),
                                ),
                            ]),
                        }),
                        CmsAdminPageBlock::SharedReference(CmsAdminSharedBlockReference {
                            id: "block-shared-store-hours".to_string(),
                            shared_block_id: "shared-store-hours".to_string(),
                            label: Some("Shared store hours".to_string()),
                            enabled: true,
                        }),
                    ],
                },
                live: Some(CmsAdminPageRevision {
                    title: "Visit Harbor".to_string(),
                    slug: "visit-harbor".to_string(),
                    summary: "Store hours, pickup guidance, and the best way to plan an in-person visit.".to_string(),
                    body_html: "<p>Visit Shoppr for coastal goods, memberships, and event collections.</p><p>Use this sample page to verify editorial draft, preview, and publish workflows before customer migration.</p>".to_string(),
                    settings: CmsAdminPageSettings {
                        page_type: "landing_page".to_string(),
                        template: Some("pages/landing-page".to_string()),
                        seo_title: Some("Visit Shoppr".to_string()),
                        seo_description: Some("Store hours and planning guidance for in-person visits.".to_string()),
                        options: CmsAdminPageOptions {
                            show_in_navigation: true,
                            allow_indexing: true,
                            localized: false,
                        },
                    },
                    blocks: vec![
                        CmsAdminPageBlock::Instance(CmsAdminBlockInstance {
                            id: "block-visit-hero".to_string(),
                            block_type: "hero".to_string(),
                            label: Some("Visit hero".to_string()),
                            enabled: true,
                            fields: BTreeMap::from([
                                ("heading".to_string(), "Visit Shoppr".to_string()),
                                (
                                    "body".to_string(),
                                    "Store hours, pickup guidance, and the best time to visit."
                                        .to_string(),
                                ),
                            ]),
                        }),
                        CmsAdminPageBlock::SharedReference(CmsAdminSharedBlockReference {
                            id: "block-shared-store-hours".to_string(),
                            shared_block_id: "shared-store-hours".to_string(),
                            label: Some("Shared store hours".to_string()),
                            enabled: true,
                        }),
                    ],
                }),
                previous_live: None,
                scheduled_publish_at: None,
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
                    settings: CmsAdminPageSettings {
                        page_type: "guide".to_string(),
                        template: Some("pages/guide".to_string()),
                        seo_title: Some("Membership Guide".to_string()),
                        seo_description: Some("What a Harbor membership purchase unlocks.".to_string()),
                        options: CmsAdminPageOptions {
                            show_in_navigation: false,
                            allow_indexing: true,
                            localized: false,
                        },
                    },
                    blocks: default_blocks_from_body_html("<p>Membership purchases appear in the account area after checkout and become active when payment capture completes.</p>"),
                },
                live: None,
                previous_live: None,
                scheduled_publish_at: None,
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
                href: "/shop".to_string(),
            },
            CmsAdminNavigationItem {
                label: "Collections".to_string(),
                href: "/shop/collections".to_string(),
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
        shared_blocks: vec![CmsAdminSharedBlock {
            id: "shared-store-hours".to_string(),
            label: "Store hours".to_string(),
            block_type: "store_hours".to_string(),
            fields: BTreeMap::from([
                ("title".to_string(), "Shop hours".to_string()),
                (
                    "body".to_string(),
                    "Monday to Saturday, 10am to 6pm. Sunday, 11am to 4pm.".to_string(),
                ),
            ]),
            updated_at: 1,
        }],
        global_settings: default_global_settings(),
    }
}

fn default_page_settings() -> CmsAdminPageSettings {
    CmsAdminPageSettings {
        page_type: "page".to_string(),
        template: None,
        seo_title: None,
        seo_description: None,
        options: CmsAdminPageOptions {
            show_in_navigation: false,
            allow_indexing: true,
            localized: false,
        },
    }
}

fn default_true() -> bool {
    true
}

fn default_global_settings() -> CmsAdminGlobalSettings {
    CmsAdminGlobalSettings {
        footer_heading: "Plan your next visit".to_string(),
        footer_body:
            "Store hours, pickup guidance, events, and membership help all live in one place."
                .to_string(),
        contact_email: "hello@shoppr.local".to_string(),
        contact_phone: "+44 20 7946 0958".to_string(),
        announcement_title: "Membership week".to_string(),
        announcement_body:
            "New members unlock early access to curated drops and event reservations.".to_string(),
    }
}

fn default_blocks_from_body_html(body_html: &str) -> Vec<CmsAdminPageBlock> {
    let trimmed = body_html.trim();
    if trimmed.is_empty() {
        Vec::new()
    } else {
        vec![CmsAdminPageBlock::Instance(CmsAdminBlockInstance {
            id: "block-body".to_string(),
            block_type: "rich_text".to_string(),
            label: Some("Body".to_string()),
            enabled: true,
            fields: BTreeMap::from([("html".to_string(), trimmed.to_string())]),
        })]
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

fn validate_page_settings(settings: CmsAdminPageSettings) -> Result<CmsAdminPageSettings, String> {
    let page_type = require_non_empty("page_settings.page_type", settings.page_type)?;
    let template = match settings.template {
        Some(template) => Some(require_non_empty("page_settings.template", template)?),
        None => None,
    };
    let seo_title = match settings.seo_title {
        Some(title) => Some(require_non_empty("page_settings.seo_title", title)?),
        None => None,
    };
    let seo_description = match settings.seo_description {
        Some(description) => Some(require_non_empty(
            "page_settings.seo_description",
            description,
        )?),
        None => None,
    };
    Ok(CmsAdminPageSettings {
        page_type,
        template,
        seo_title,
        seo_description,
        options: settings.options,
    })
}

fn validate_global_settings(
    settings: CmsAdminGlobalSettings,
) -> Result<CmsAdminGlobalSettings, String> {
    let footer_heading = require_non_empty("footer_heading", settings.footer_heading)?;
    let footer_body = require_non_empty("footer_body", settings.footer_body)?;
    let contact_email = settings.contact_email.trim().to_string();
    if !contact_email.is_empty() && !contact_email.contains('@') {
        return Err("`contact_email` must contain `@` when provided".to_string());
    }
    Ok(CmsAdminGlobalSettings {
        footer_heading,
        footer_body,
        contact_email,
        contact_phone: settings.contact_phone.trim().to_string(),
        announcement_title: settings.announcement_title.trim().to_string(),
        announcement_body: settings.announcement_body.trim().to_string(),
    })
}

fn validate_page_blocks(
    blocks: &[CmsAdminPageBlock],
    shared_blocks: &[CmsAdminSharedBlock],
) -> Result<(), String> {
    let mut ids = BTreeMap::new();
    for block in blocks {
        match block {
            CmsAdminPageBlock::Instance(instance) => {
                let id = require_non_empty("page_block.id", instance.id.clone())?;
                require_non_empty("page_block.block_type", instance.block_type.clone())?;
                if ids.insert(id.clone(), true).is_some() {
                    return Err(format!("duplicate page block id `{id}`"));
                }
            }
            CmsAdminPageBlock::SharedReference(reference) => {
                let id = require_non_empty("shared_block_reference.id", reference.id.clone())?;
                let shared_block_id = require_non_empty(
                    "shared_block_reference.shared_block_id",
                    reference.shared_block_id.clone(),
                )?;
                if ids.insert(id.clone(), true).is_some() {
                    return Err(format!("duplicate page block id `{id}`"));
                }
                if !shared_blocks
                    .iter()
                    .any(|block| block.id == shared_block_id)
                {
                    return Err(format!(
                        "shared block `{shared_block_id}` was not found for page block `{id}`"
                    ));
                }
            }
        }
    }
    Ok(())
}

fn validate_shared_block(block: CmsAdminSharedBlock) -> Result<CmsAdminSharedBlock, String> {
    Ok(CmsAdminSharedBlock {
        id: require_non_empty("shared_block.id", block.id)?,
        label: require_non_empty("shared_block.label", block.label)?,
        block_type: require_non_empty("shared_block.block_type", block.block_type)?,
        fields: block.fields,
        updated_at: block.updated_at,
    })
}

impl CmsAdminPageBlock {
    pub(crate) fn id(&self) -> &str {
        match self {
            Self::Instance(instance) => instance.id.as_str(),
            Self::SharedReference(reference) => reference.id.as_str(),
        }
    }

    fn set_id(&mut self, id: String) {
        match self {
            Self::Instance(instance) => instance.id = id,
            Self::SharedReference(reference) => reference.id = id,
        }
    }
}

fn next_duplicate_block_id(blocks: &[CmsAdminPageBlock], source_id: &str) -> String {
    let mut index = 2;
    let base = format!("{source_id}-copy");
    if !blocks.iter().any(|block| block.id() == base) {
        return base;
    }
    loop {
        let candidate = format!("{source_id}-copy-{index}");
        if !blocks.iter().any(|block| block.id() == candidate) {
            return candidate;
        }
        index += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_workspace_exposes_structured_page_settings_and_shared_blocks() {
        let workspace = default_workspace();
        let visit_page = workspace
            .pages
            .iter()
            .find(|page| page.id == "page-visit-harbor")
            .unwrap();

        assert_eq!(visit_page.draft.settings.page_type, "landing_page");
        assert_eq!(
            visit_page.draft.settings.template.as_deref(),
            Some("pages/landing-page")
        );
        assert_eq!(visit_page.draft.blocks.len(), 2);
        assert!(matches!(
            &visit_page.draft.blocks[1],
            CmsAdminPageBlock::SharedReference(reference)
                if reference.shared_block_id == "shared-store-hours"
        ));
        assert_eq!(workspace.shared_blocks.len(), 1);
        assert_eq!(workspace.shared_blocks[0].block_type, "store_hours");
        assert_eq!(
            workspace.global_settings.footer_heading,
            "Plan your next visit"
        );
        assert_eq!(
            workspace.global_settings.contact_email,
            "hello@shoppr.local"
        );
    }

    #[test]
    fn save_page_draft_preserves_structured_settings_and_blocks_for_existing_pages() {
        let mut workspace = default_workspace();
        let original = workspace
            .pages
            .iter()
            .find(|page| page.id == "page-visit-harbor")
            .unwrap()
            .draft
            .clone();

        let page_id = workspace
            .save_page_draft(
                CmsAdminPageInput {
                    page_id: Some("page-visit-harbor".to_string()),
                    title: "Visit Harbor Updated".to_string(),
                    slug: "visit-harbor".to_string(),
                    summary: "Updated summary".to_string(),
                    body_html: "<p>Updated body</p>".to_string(),
                },
                42,
            )
            .unwrap();

        let page = workspace
            .pages
            .iter()
            .find(|page| page.id == page_id)
            .unwrap();
        assert_eq!(page.draft.title, "Visit Harbor Updated");
        assert_eq!(page.draft.summary, "Updated summary");
        assert_eq!(page.draft.body_html, "<p>Updated body</p>");
        assert_eq!(page.draft.settings, original.settings);
        assert_eq!(page.draft.blocks, original.blocks);
    }

    #[test]
    fn save_page_draft_refreshes_legacy_compatible_blocks_for_existing_pages() {
        let mut workspace = default_workspace();

        let page_id = workspace
            .save_page_draft(
                CmsAdminPageInput {
                    page_id: Some("page-membership-guide".to_string()),
                    title: "Membership Guide".to_string(),
                    slug: "membership-guide".to_string(),
                    summary: "Updated summary".to_string(),
                    body_html: "<p>Membership purchases appear in the account area after checkout.</p><p>Publishing from Shoppr admin makes this page live for the storefront.</p>".to_string(),
                },
                42,
            )
            .unwrap();

        let page = workspace
            .pages
            .iter()
            .find(|page| page.id == page_id)
            .unwrap();
        assert_eq!(
            page.draft.blocks,
            default_blocks_from_body_html(
                "<p>Membership purchases appear in the account area after checkout.</p><p>Publishing from Shoppr admin makes this page live for the storefront.</p>"
            )
        );
    }

    #[test]
    fn replace_page_blocks_requires_existing_shared_block_targets() {
        let mut workspace = default_workspace();
        let error = workspace
            .replace_page_blocks(
                "page-membership-guide",
                vec![CmsAdminPageBlock::SharedReference(
                    CmsAdminSharedBlockReference {
                        id: "block-missing-shared".to_string(),
                        shared_block_id: "missing-shared".to_string(),
                        label: Some("Missing".to_string()),
                        enabled: true,
                    },
                )],
                55,
            )
            .unwrap_err();

        assert!(
            error.contains("shared block `missing-shared` was not found"),
            "{error}"
        );
    }

    #[test]
    fn save_global_settings_validates_and_updates_workspace_state() {
        let mut workspace = default_workspace();

        let error = workspace
            .save_global_settings(CmsAdminGlobalSettings {
                footer_heading: "".to_string(),
                footer_body: "Body".to_string(),
                contact_email: "invalid-email".to_string(),
                contact_phone: "".to_string(),
                announcement_title: "".to_string(),
                announcement_body: "".to_string(),
            })
            .unwrap_err();
        assert!(error.contains("footer_heading") || error.contains("contact_email"));

        workspace
            .save_global_settings(CmsAdminGlobalSettings {
                footer_heading: "Plan a store visit".to_string(),
                footer_body: "Pickup guidance, store hours, and event support.".to_string(),
                contact_email: "concierge@example.com".to_string(),
                contact_phone: "+44 20 7000 0000".to_string(),
                announcement_title: "Members week".to_string(),
                announcement_body: "Priority booking opens on Monday.".to_string(),
            })
            .unwrap();

        assert_eq!(
            workspace.global_settings.footer_heading,
            "Plan a store visit"
        );
        assert_eq!(
            workspace.global_settings.contact_email,
            "concierge@example.com"
        );
        assert_eq!(workspace.global_settings.announcement_title, "Members week");
    }

    #[test]
    fn save_settings_blocks_and_shared_blocks_updates_workspace_state() {
        let mut workspace = default_workspace();

        workspace
            .save_shared_block(CmsAdminSharedBlock {
                id: "shared-membership-cta".to_string(),
                label: "Membership CTA".to_string(),
                block_type: "callout".to_string(),
                fields: BTreeMap::from([
                    ("heading".to_string(), "Join today".to_string()),
                    ("cta_href".to_string(), "/shop".to_string()),
                ]),
                updated_at: 99,
            })
            .unwrap();

        workspace
            .save_page_settings(
                "page-membership-guide",
                CmsAdminPageSettings {
                    page_type: "membership_guide".to_string(),
                    template: Some("pages/membership-guide".to_string()),
                    seo_title: Some("Membership Guide | Shoppr".to_string()),
                    seo_description: Some("Everything included with membership.".to_string()),
                    options: CmsAdminPageOptions {
                        show_in_navigation: true,
                        allow_indexing: false,
                        localized: true,
                    },
                },
                100,
            )
            .unwrap();

        workspace
            .replace_page_blocks(
                "page-membership-guide",
                vec![
                    CmsAdminPageBlock::Instance(CmsAdminBlockInstance {
                        id: "block-membership-hero".to_string(),
                        block_type: "hero".to_string(),
                        label: Some("Hero".to_string()),
                        enabled: true,
                        fields: BTreeMap::from([(
                            "heading".to_string(),
                            "Membership Guide".to_string(),
                        )]),
                    }),
                    CmsAdminPageBlock::SharedReference(CmsAdminSharedBlockReference {
                        id: "block-membership-cta".to_string(),
                        shared_block_id: "shared-membership-cta".to_string(),
                        label: Some("CTA".to_string()),
                        enabled: true,
                    }),
                ],
                101,
            )
            .unwrap();

        workspace
            .publish_page("page-membership-guide", 102)
            .unwrap();

        let page = workspace
            .pages
            .iter()
            .find(|page| page.id == "page-membership-guide")
            .unwrap();
        assert_eq!(page.status(), CmsAdminPageStatus::Published);
        assert_eq!(page.draft.settings.page_type, "membership_guide");
        assert!(!page.draft.settings.options.allow_indexing);
        assert_eq!(page.draft.blocks.len(), 2);
        assert_eq!(page.live.as_ref().unwrap().blocks, page.draft.blocks);
        assert_eq!(page.live.as_ref().unwrap().settings, page.draft.settings);
    }

    #[test]
    fn scheduling_and_rollback_update_page_workflow_state() {
        let mut workspace = default_workspace();

        workspace
            .publish_page("page-membership-guide", 100)
            .expect("page should publish");
        workspace
            .save_page_draft(
                CmsAdminPageInput {
                    page_id: Some("page-membership-guide".to_string()),
                    title: "Membership Guide Updated".to_string(),
                    slug: "membership-guide".to_string(),
                    summary: "Updated summary".to_string(),
                    body_html: "<p>Updated body</p>".to_string(),
                },
                101,
            )
            .expect("draft should update");
        workspace
            .schedule_page_publish("page-membership-guide", 150, 100, 102)
            .expect("page should schedule");

        let page = workspace
            .pages
            .iter()
            .find(|page| page.id == "page-membership-guide")
            .expect("page should exist");
        assert_eq!(
            page.status(),
            CmsAdminPageStatus::PublishedWithScheduledDraft
        );
        assert_eq!(page.scheduled_publish_at, Some(150));

        let applied = workspace.apply_due_schedules(150);
        assert_eq!(applied, vec!["page-membership-guide".to_string()]);

        let page = workspace
            .pages
            .iter()
            .find(|page| page.id == "page-membership-guide")
            .expect("page should exist");
        assert_eq!(page.status(), CmsAdminPageStatus::Published);
        assert!(page.has_rollback_target());
        assert_eq!(
            page.live.as_ref().expect("live page").title,
            "Membership Guide Updated"
        );

        workspace
            .rollback_page("page-membership-guide", 151)
            .expect("rollback should succeed");

        let page = workspace
            .pages
            .iter()
            .find(|page| page.id == "page-membership-guide")
            .expect("page should exist");
        assert_eq!(
            page.live.as_ref().expect("live page").title,
            "Membership Guide"
        );
        assert_eq!(page.draft.title, "Membership Guide");
    }

    #[test]
    fn duplicate_page_block_inserts_copy_after_source_with_unique_id() {
        let mut workspace = default_workspace();

        let duplicated_id = workspace
            .duplicate_page_block("page-visit-harbor", "block-visit-hero", 77)
            .expect("block should duplicate");

        let page = workspace
            .pages
            .iter()
            .find(|page| page.id == "page-visit-harbor")
            .expect("page should exist");
        assert_eq!(duplicated_id, "block-visit-hero-copy");
        assert_eq!(page.draft.blocks.len(), 3);
        assert_eq!(page.draft.blocks[1].id(), "block-visit-hero-copy");
    }
}
