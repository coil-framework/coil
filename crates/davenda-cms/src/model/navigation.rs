use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationTarget {
    Page(PageId),
    ExternalUrl(String),
}

impl NavigationTarget {
    pub fn external(url: impl Into<String>) -> Result<Self, CmsModelError> {
        Ok(Self::ExternalUrl(validate_path(
            "external_url",
            url.into(),
        )?))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationItem {
    pub id: NavigationItemId,
    pub label: String,
    pub target: NavigationTarget,
    pub children: Vec<NavigationItem>,
}

impl NavigationItem {
    pub fn page(
        id: NavigationItemId,
        label: impl Into<String>,
        page_id: PageId,
    ) -> Result<Self, CmsModelError> {
        Ok(Self {
            id,
            label: require_non_empty("navigation_label", label.into())?,
            target: NavigationTarget::Page(page_id),
            children: Vec::new(),
        })
    }

    pub fn external(
        id: NavigationItemId,
        label: impl Into<String>,
        url: impl Into<String>,
    ) -> Result<Self, CmsModelError> {
        Ok(Self {
            id,
            label: require_non_empty("navigation_label", label.into())?,
            target: NavigationTarget::external(url)?,
            children: Vec::new(),
        })
    }

    pub fn with_child(mut self, child: NavigationItem) -> Self {
        self.children.push(child);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedNavigationItem {
    pub id: NavigationItemId,
    pub label: String,
    pub href: String,
    pub children: Vec<ResolvedNavigationItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationTree {
    pub id: NavigationId,
    pub items: Vec<NavigationItem>,
}

impl NavigationTree {
    pub fn new(id: NavigationId, items: Vec<NavigationItem>) -> Result<Self, CmsModelError> {
        let tree = Self { id, items };
        tree.validate()?;
        Ok(tree)
    }

    pub fn validate(&self) -> Result<(), CmsModelError> {
        let mut seen = BTreeSet::new();
        for item in &self.items {
            validate_navigation_item(item, &mut Vec::new(), &mut seen)?;
        }
        Ok(())
    }

    pub fn resolve(
        &self,
        pages: &BTreeMap<PageId, CmsPage>,
    ) -> Result<Vec<ResolvedNavigationItem>, CmsModelError> {
        let mut resolved = Vec::new();
        for item in &self.items {
            if let Some(item) = resolve_navigation_item(item, pages)? {
                resolved.push(item);
            }
        }
        Ok(resolved)
    }
}

fn validate_navigation_item(
    item: &NavigationItem,
    stack: &mut Vec<NavigationItemId>,
    seen: &mut BTreeSet<NavigationItemId>,
) -> Result<(), CmsModelError> {
    if stack.contains(&item.id) {
        return Err(CmsModelError::NavigationCycle {
            item_id: item.id.to_string(),
        });
    }

    if !seen.insert(item.id.clone()) {
        return Err(CmsModelError::DuplicateNavigationItem {
            item_id: item.id.to_string(),
        });
    }

    stack.push(item.id.clone());
    for child in &item.children {
        validate_navigation_item(child, stack, seen)?;
    }
    stack.pop();

    Ok(())
}

fn resolve_navigation_item(
    item: &NavigationItem,
    pages: &BTreeMap<PageId, CmsPage>,
) -> Result<Option<ResolvedNavigationItem>, CmsModelError> {
    let href = match &item.target {
        NavigationTarget::ExternalUrl(url) => url.clone(),
        NavigationTarget::Page(page_id) => match pages.get(page_id) {
            Some(page) if page.publication().is_live() => page.live_path()?,
            _ => return Ok(None),
        },
    };

    let mut children = Vec::new();
    for child in &item.children {
        if let Some(child) = resolve_navigation_item(child, pages)? {
            children.push(child);
        }
    }

    Ok(Some(ResolvedNavigationItem {
        id: item.id.clone(),
        label: item.label.clone(),
        href,
        children,
    }))
}
