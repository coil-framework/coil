use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;

use crate::validation::require_non_empty;
use crate::{I18nError, validation::validate_token};

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
}
