use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use davenda_i18n::LocalizedUrls;

use crate::SeoError;
use crate::validation::{require_non_empty, validate_absolute_url};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RobotsDirective {
    Index,
    NoIndex,
    Follow,
    NoFollow,
    NoArchive,
}

impl fmt::Display for RobotsDirective {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Index => f.write_str("index"),
            Self::NoIndex => f.write_str("noindex"),
            Self::Follow => f.write_str("follow"),
            Self::NoFollow => f.write_str("nofollow"),
            Self::NoArchive => f.write_str("noarchive"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenGraphType {
    Website,
    Article,
    Product,
    Event,
}

impl fmt::Display for OpenGraphType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Website => f.write_str("website"),
            Self::Article => f.write_str("article"),
            Self::Product => f.write_str("product"),
            Self::Event => f.write_str("event"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenGraphData {
    pub title: String,
    pub description: String,
    pub image_url: Option<String>,
    pub graph_type: OpenGraphType,
}

impl OpenGraphData {
    pub fn new(
        title: impl Into<String>,
        description: impl Into<String>,
        image_url: Option<String>,
        graph_type: OpenGraphType,
    ) -> Result<Self, SeoError> {
        Ok(Self {
            title: require_non_empty("og_title", title.into())?,
            description: require_non_empty("og_description", description.into())?,
            image_url: image_url
                .map(|url| validate_absolute_url("og_image_url", url))
                .transpose()?,
            graph_type,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadMetadata {
    pub title: String,
    pub description: String,
    pub canonical_url: String,
    pub alternate_urls: BTreeMap<String, String>,
    pub robots: BTreeSet<RobotsDirective>,
    pub open_graph: Option<OpenGraphData>,
}

impl HeadMetadata {
    pub fn new(
        title: impl Into<String>,
        description: impl Into<String>,
        urls: LocalizedUrls,
        robots: impl IntoIterator<Item = RobotsDirective>,
        open_graph: Option<OpenGraphData>,
    ) -> Result<Self, SeoError> {
        Ok(Self {
            title: require_non_empty("title", title.into())?,
            description: require_non_empty("description", description.into())?,
            canonical_url: validate_absolute_url("canonical_url", urls.canonical)?,
            alternate_urls: urls.alternate_hreflang,
            robots: robots.into_iter().collect(),
            open_graph,
        })
    }

    pub fn robots_content(&self) -> String {
        self.robots
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",")
    }
}
