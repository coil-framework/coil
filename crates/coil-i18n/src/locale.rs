use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::fs;
use std::path::Path;

use crate::validation::require_non_empty;
use crate::{I18nError, TranslationCatalogLoadError, validation::validate_token};

macro_rules! token_type {
    ($name:ident, $field:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, I18nError> {
                Ok(Self(validate_token($field, value.into())?))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

token_type!(LocaleTag, "locale");
token_type!(CurrencyCode, "currency");
token_type!(TimeZoneId, "timezone");
token_type!(MessageKey, "message_key");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocaleContext {
    pub locale: LocaleTag,
    pub fallback_chain: Vec<LocaleTag>,
    pub currency: CurrencyCode,
    pub timezone: TimeZoneId,
}

impl LocaleContext {
    pub fn new(
        locale: LocaleTag,
        fallback_chain: Vec<LocaleTag>,
        currency: CurrencyCode,
        timezone: TimeZoneId,
    ) -> Self {
        Self {
            locale,
            fallback_chain,
            currency,
            timezone,
        }
    }

    pub fn locale_candidates(&self) -> Vec<&LocaleTag> {
        let mut seen = BTreeSet::new();
        let mut locales = Vec::new();

        if seen.insert(self.locale.as_str()) {
            locales.push(&self.locale);
        }

        for fallback in &self.fallback_chain {
            if seen.insert(fallback.as_str()) {
                locales.push(fallback);
            }
        }

        locales
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationCatalog {
    pub locale: LocaleTag,
    messages: BTreeMap<MessageKey, String>,
}

impl TranslationCatalog {
    pub fn new(
        locale: LocaleTag,
        messages: impl IntoIterator<Item = (MessageKey, String)>,
    ) -> Result<Self, I18nError> {
        let mut map = BTreeMap::new();
        for (key, value) in messages {
            if map.contains_key(&key) {
                return Err(I18nError::DuplicateMessageKey {
                    locale: locale.to_string(),
                    key: key.to_string(),
                });
            }
            map.insert(key, require_non_empty("message_value", value)?);
        }

        Ok(Self {
            locale,
            messages: map,
        })
    }

    pub fn translate(&self, key: &MessageKey) -> Option<&str> {
        self.messages.get(key).map(String::as_str)
    }

    pub fn locale(&self) -> &LocaleTag {
        &self.locale
    }

    pub fn messages(&self) -> impl Iterator<Item = (&MessageKey, &str)> {
        self.messages
            .iter()
            .map(|(key, value)| (key, value.as_str()))
    }

    pub fn from_toml_file(
        locale: LocaleTag,
        path: impl AsRef<Path>,
    ) -> Result<Self, TranslationCatalogLoadError> {
        let path = path.as_ref();
        let source =
            fs::read_to_string(path).map_err(|error| TranslationCatalogLoadError::Read {
                path: path.display().to_string(),
                reason: error.to_string(),
            })?;
        Self::from_toml_str_with_source(locale, &source, path.display().to_string())
    }

    pub fn from_toml_str(
        locale: LocaleTag,
        input: &str,
    ) -> Result<Self, TranslationCatalogLoadError> {
        Self::from_toml_str_with_source(locale, input, "<inline>".to_string())
    }

    fn from_toml_str_with_source(
        locale: LocaleTag,
        input: &str,
        source_name: String,
    ) -> Result<Self, TranslationCatalogLoadError> {
        let value: toml::Value =
            toml::from_str(input).map_err(|error| TranslationCatalogLoadError::Parse {
                path: source_name.clone(),
                reason: error.to_string(),
            })?;
        let table = value
            .as_table()
            .ok_or_else(|| TranslationCatalogLoadError::Parse {
                path: source_name.clone(),
                reason: "translation catalogs must be TOML tables of string messages".to_string(),
            })?;
        let mut messages = Vec::new();
        flatten_toml_translation_table(table, &mut Vec::new(), &mut messages, &source_name)?;
        Self::new(locale, messages).map_err(|error| TranslationCatalogLoadError::Parse {
            path: source_name,
            reason: error.to_string(),
        })
    }
}

fn flatten_toml_translation_table(
    table: &toml::value::Table,
    segments: &mut Vec<String>,
    messages: &mut Vec<(MessageKey, String)>,
    source_name: &str,
) -> Result<(), TranslationCatalogLoadError> {
    for (key, value) in table {
        segments.push(key.clone());
        match value {
            toml::Value::String(message) => {
                let message_key = MessageKey::new(segments.join(".")).map_err(|error| {
                    TranslationCatalogLoadError::Parse {
                        path: source_name.to_string(),
                        reason: error.to_string(),
                    }
                })?;
                messages.push((message_key, message.clone()));
            }
            toml::Value::Table(nested) => {
                flatten_toml_translation_table(nested, segments, messages, source_name)?;
            }
            other => {
                let key = segments.join(".");
                return Err(TranslationCatalogLoadError::Parse {
                    path: source_name.to_string(),
                    reason: format!(
                        "translation key `{key}` must map to a string value, found `{}`",
                        toml_type_name(other)
                    ),
                });
            }
        }
        segments.pop();
    }

    Ok(())
}

fn toml_type_name(value: &toml::Value) -> &'static str {
    match value {
        toml::Value::String(_) => "string",
        toml::Value::Integer(_) => "integer",
        toml::Value::Float(_) => "float",
        toml::Value::Boolean(_) => "boolean",
        toml::Value::Datetime(_) => "datetime",
        toml::Value::Array(_) => "array",
        toml::Value::Table(_) => "table",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationRuntime {
    default_locale: LocaleTag,
    catalogs: HashMap<LocaleTag, TranslationCatalog>,
}

impl TranslationRuntime {
    pub fn new(
        default_locale: LocaleTag,
        catalogs: impl IntoIterator<Item = TranslationCatalog>,
    ) -> Result<Self, I18nError> {
        let mut map = HashMap::new();
        for catalog in catalogs {
            map.insert(catalog.locale.clone(), catalog);
        }

        if !map.contains_key(&default_locale) {
            return Err(I18nError::MissingLocale {
                locale: default_locale.to_string(),
            });
        }

        Ok(Self {
            default_locale,
            catalogs: map,
        })
    }

    pub fn translate(
        &self,
        context: &LocaleContext,
        key: &MessageKey,
    ) -> Result<String, I18nError> {
        for locale in context.locale_candidates() {
            if let Some(catalog) = self.catalogs.get(locale) {
                if let Some(value) = catalog.translate(key) {
                    return Ok(value.to_string());
                }
            }
        }

        let default_catalog =
            self.catalogs
                .get(&self.default_locale)
                .ok_or_else(|| I18nError::MissingLocale {
                    locale: self.default_locale.to_string(),
                })?;

        Ok(default_catalog
            .translate(key)
            .unwrap_or(key.as_str())
            .to_string())
    }

    pub fn resolved_messages(&self, context: &LocaleContext) -> BTreeMap<MessageKey, String> {
        let mut resolved = BTreeMap::new();

        if let Some(default_catalog) = self.catalogs.get(&self.default_locale) {
            for (key, value) in default_catalog.messages() {
                resolved.insert(key.clone(), value.to_string());
            }
        }

        let mut overlay_locales = context
            .locale_candidates()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        overlay_locales.reverse();

        for locale in overlay_locales {
            if let Some(catalog) = self.catalogs.get(&locale) {
                for (key, value) in catalog.messages() {
                    resolved.insert(key.clone(), value.to_string());
                }
            }
        }

        resolved
    }
}
