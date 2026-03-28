use std::collections::BTreeMap;
use std::fmt;

use coil_i18n::LocaleTag;

use crate::SeoError;
use crate::validation::validate_absolute_url;

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
