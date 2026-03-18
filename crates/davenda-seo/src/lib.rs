use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use davenda_i18n::{LocaleTag, LocalizedUrls};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeoError {
    EmptyField { field: &'static str },
    InvalidUrl { field: &'static str, value: String },
    InvalidJsonLdProperty { property: String },
    DuplicateJsonLdProperty { property: String },
}

impl fmt::Display for SeoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(f, "`{field}` cannot be empty"),
            Self::InvalidUrl { field, value } => {
                write!(f, "`{field}` must be an absolute URL, got `{value}`")
            }
            Self::InvalidJsonLdProperty { property } => {
                write!(f, "JSON-LD property `{property}` is invalid")
            }
            Self::DuplicateJsonLdProperty { property } => {
                write!(f, "JSON-LD property `{property}` is duplicated")
            }
        }
    }
}

impl Error for SeoError {}

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SitemapChangeFrequency {
    Always,
    Hourly,
    Daily,
    Weekly,
    Monthly,
    Yearly,
    Never,
}

impl fmt::Display for SitemapChangeFrequency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Always => f.write_str("always"),
            Self::Hourly => f.write_str("hourly"),
            Self::Daily => f.write_str("daily"),
            Self::Weekly => f.write_str("weekly"),
            Self::Monthly => f.write_str("monthly"),
            Self::Yearly => f.write_str("yearly"),
            Self::Never => f.write_str("never"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SitemapImage {
    pub url: String,
    pub caption: Option<String>,
}

impl SitemapImage {
    pub fn new(url: impl Into<String>, caption: Option<String>) -> Result<Self, SeoError> {
        Ok(Self {
            url: validate_absolute_url("image_url", url.into())?,
            caption: caption.map(|caption| caption.trim().to_string()),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SitemapEntry {
    pub loc: String,
    pub last_modified_unix: i64,
    pub change_frequency: SitemapChangeFrequency,
    pub priority: f32,
    pub alternates: BTreeMap<String, String>,
    pub images: Vec<SitemapImage>,
}

impl SitemapEntry {
    pub fn new(
        loc: impl Into<String>,
        last_modified_unix: i64,
        change_frequency: SitemapChangeFrequency,
        priority: f32,
        alternates: BTreeMap<String, String>,
        images: Vec<SitemapImage>,
    ) -> Result<Self, SeoError> {
        Ok(Self {
            loc: validate_absolute_url("loc", loc.into())?,
            last_modified_unix,
            change_frequency,
            priority,
            alternates,
            images,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SitemapDocument {
    entries: Vec<SitemapEntry>,
}

impl SitemapDocument {
    pub fn new(entries: Vec<SitemapEntry>) -> Self {
        Self { entries }
    }

    pub fn entries(&self) -> &[SitemapEntry] {
        &self.entries
    }

    pub fn localized_entries_for(&self, locale: &LocaleTag) -> Vec<&SitemapEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.alternates.contains_key(locale.as_str()))
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum JsonLdValue {
    String(String),
    Number(f64),
    Bool(bool),
    Node(JsonLdNode),
    List(Vec<JsonLdValue>),
}

impl JsonLdValue {
    fn render(&self) -> String {
        match self {
            Self::String(value) => format!("\"{}\"", escape_json(value)),
            Self::Number(value) => value.to_string(),
            Self::Bool(value) => value.to_string(),
            Self::Node(node) => node.render(),
            Self::List(values) => format!(
                "[{}]",
                values
                    .iter()
                    .map(JsonLdValue::render)
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct JsonLdNode {
    schema_type: String,
    properties: BTreeMap<String, JsonLdValue>,
}

impl JsonLdNode {
    pub fn new(schema_type: impl Into<String>) -> Result<Self, SeoError> {
        Ok(Self {
            schema_type: require_non_empty("schema_type", schema_type.into())?,
            properties: BTreeMap::new(),
        })
    }

    pub fn set_string(
        mut self,
        property: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, SeoError> {
        let property = validate_property_name(property.into())?;
        if self.properties.contains_key(&property) {
            return Err(SeoError::DuplicateJsonLdProperty { property });
        }
        self.properties.insert(
            property,
            JsonLdValue::String(require_non_empty("json_ld_string", value.into())?),
        );
        Ok(self)
    }

    pub fn set_number(mut self, property: impl Into<String>, value: f64) -> Result<Self, SeoError> {
        let property = validate_property_name(property.into())?;
        if self.properties.contains_key(&property) {
            return Err(SeoError::DuplicateJsonLdProperty { property });
        }
        self.properties.insert(property, JsonLdValue::Number(value));
        Ok(self)
    }

    pub fn set_bool(mut self, property: impl Into<String>, value: bool) -> Result<Self, SeoError> {
        let property = validate_property_name(property.into())?;
        if self.properties.contains_key(&property) {
            return Err(SeoError::DuplicateJsonLdProperty { property });
        }
        self.properties.insert(property, JsonLdValue::Bool(value));
        Ok(self)
    }

    pub fn set_node(
        mut self,
        property: impl Into<String>,
        node: JsonLdNode,
    ) -> Result<Self, SeoError> {
        let property = validate_property_name(property.into())?;
        if self.properties.contains_key(&property) {
            return Err(SeoError::DuplicateJsonLdProperty { property });
        }
        self.properties.insert(property, JsonLdValue::Node(node));
        Ok(self)
    }

    pub fn set_list(
        mut self,
        property: impl Into<String>,
        values: Vec<JsonLdValue>,
    ) -> Result<Self, SeoError> {
        let property = validate_property_name(property.into())?;
        if self.properties.contains_key(&property) {
            return Err(SeoError::DuplicateJsonLdProperty { property });
        }
        self.properties.insert(property, JsonLdValue::List(values));
        Ok(self)
    }

    pub fn render(&self) -> String {
        let mut segments = vec![format!("\"@type\":\"{}\"", escape_json(&self.schema_type))];
        for (property, value) in &self.properties {
            segments.push(format!("\"{}\":{}", escape_json(property), value.render()));
        }
        format!("{{{}}}", segments.join(","))
    }
}

pub fn page_node(
    name: impl Into<String>,
    url: impl Into<String>,
    description: impl Into<String>,
) -> Result<JsonLdNode, SeoError> {
    JsonLdNode::new("WebPage")?
        .set_string("name", name.into())?
        .set_string("url", validate_absolute_url("url", url.into())?)?
        .set_string("description", description.into())
}

pub fn product_node(
    name: impl Into<String>,
    url: impl Into<String>,
    price: f64,
    currency: impl Into<String>,
    availability: impl Into<String>,
) -> Result<JsonLdNode, SeoError> {
    let offer = JsonLdNode::new("Offer")?
        .set_number("price", price)?
        .set_string("priceCurrency", currency.into())?
        .set_string("availability", availability.into())?;

    JsonLdNode::new("Product")?
        .set_string("name", name.into())?
        .set_string("url", validate_absolute_url("url", url.into())?)?
        .set_node("offers", offer)
}

pub fn event_node(
    name: impl Into<String>,
    url: impl Into<String>,
    start_date_unix: i64,
    location_name: impl Into<String>,
) -> Result<JsonLdNode, SeoError> {
    let location = JsonLdNode::new("Place")?.set_string("name", location_name.into())?;

    JsonLdNode::new("Event")?
        .set_string("name", name.into())?
        .set_string("url", validate_absolute_url("url", url.into())?)?
        .set_number("startDateUnix", start_date_unix as f64)?
        .set_node("location", location)
}

fn validate_absolute_url(field: &'static str, value: String) -> Result<String, SeoError> {
    let trimmed = require_non_empty(field, value)?;
    if trimmed.starts_with("https://") || trimmed.starts_with("http://") {
        Ok(trimmed)
    } else {
        Err(SeoError::InvalidUrl {
            field,
            value: trimmed,
        })
    }
}

fn validate_property_name(value: String) -> Result<String, SeoError> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || !trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '@' | '_' | '-'))
    {
        Err(SeoError::InvalidJsonLdProperty {
            property: trimmed.to_string(),
        })
    } else {
        Ok(trimmed.to_string())
    }
}

fn require_non_empty(field: &'static str, value: String) -> Result<String, SeoError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(SeoError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}

fn escape_json(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use davenda_i18n::{LocaleRouter, LocaleUrlConfig};

    fn locale(locale: &str) -> LocaleTag {
        LocaleTag::new(locale).unwrap()
    }

    #[test]
    fn head_metadata_tracks_canonical_alternates_and_robots() {
        let router = LocaleRouter::new(LocaleUrlConfig::path_prefix("www.example.com").unwrap());
        let urls = router
            .alternate_urls(
                &[locale("en-GB"), locale("fr-FR")],
                "/events/spring-tasting",
            )
            .unwrap();

        let metadata = HeadMetadata::new(
            "Spring Tasting",
            "Seasonal event page",
            urls,
            [
                RobotsDirective::Index,
                RobotsDirective::Follow,
                RobotsDirective::NoArchive,
            ],
            Some(
                OpenGraphData::new(
                    "Spring Tasting",
                    "Seasonal event page",
                    Some("https://cdn.example.com/event.jpg".to_string()),
                    OpenGraphType::Event,
                )
                .unwrap(),
            ),
        )
        .unwrap();

        assert_eq!(
            metadata.canonical_url,
            "https://www.example.com/en-GB/events/spring-tasting"
        );
        assert_eq!(
            metadata.alternate_urls["fr-FR"],
            "https://www.example.com/fr-FR/events/spring-tasting"
        );
        assert_eq!(metadata.robots_content(), "index,follow,noarchive");
    }

    #[test]
    fn sitemap_document_keeps_alternate_locale_variants() {
        let mut alternates = BTreeMap::new();
        alternates.insert(
            "en-GB".to_string(),
            "https://www.example.com/en-GB/events/spring-tasting".to_string(),
        );
        alternates.insert(
            "fr-FR".to_string(),
            "https://www.example.com/fr-FR/events/spring-tasting".to_string(),
        );
        let document = SitemapDocument::new(vec![
            SitemapEntry::new(
                "https://www.example.com/en-GB/events/spring-tasting",
                1_710_000_000,
                SitemapChangeFrequency::Weekly,
                0.8,
                alternates,
                vec![
                    SitemapImage::new(
                        "https://cdn.example.com/event.jpg",
                        Some("Event image".to_string()),
                    )
                    .unwrap(),
                ],
            )
            .unwrap(),
        ]);

        let fr_entries = document.localized_entries_for(&locale("fr-FR"));
        assert_eq!(fr_entries.len(), 1);
        assert_eq!(fr_entries[0].images.len(), 1);
    }

    #[test]
    fn json_ld_builders_render_typed_product_and_event_nodes() {
        let product = product_node(
            "Gold Membership",
            "https://www.example.com/en-GB/shop/gold-membership",
            129.0,
            "GBP",
            "https://schema.org/InStock",
        )
        .unwrap();
        let event = event_node(
            "Spring Tasting",
            "https://www.example.com/en-GB/events/spring-tasting",
            1_710_000_000,
            "Davenda Hall",
        )
        .unwrap();

        let product_json = product.render();
        let event_json = event.render();
        assert!(product_json.contains("\"@type\":\"Product\""));
        assert!(product_json.contains("\"priceCurrency\":\"GBP\""));
        assert!(event_json.contains("\"@type\":\"Event\""));
        assert!(event_json.contains("\"location\""));
    }

    #[test]
    fn page_json_ld_builder_uses_absolute_urls() {
        let node = page_node(
            "Events",
            "https://www.example.com/en-GB/events",
            "Browse upcoming events",
        )
        .unwrap();

        assert!(node.render().contains("\"@type\":\"WebPage\""));
        assert!(
            node.render()
                .contains("\"url\":\"https://www.example.com/en-GB/events\"")
        );
    }
}
