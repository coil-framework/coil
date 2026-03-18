use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum I18nError {
    EmptyField { field: &'static str },
    InvalidToken { field: &'static str, value: String },
    InvalidRoute { field: &'static str, value: String },
    DuplicateMessageKey { locale: String, key: String },
    MissingLocale { locale: String },
}

impl fmt::Display for I18nError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(f, "`{field}` cannot be empty"),
            Self::InvalidToken { field, value } => {
                write!(f, "`{field}` contains an invalid token `{value}`")
            }
            Self::InvalidRoute { field, value } => {
                write!(f, "`{field}` must start with `/`, got `{value}`")
            }
            Self::DuplicateMessageKey { locale, key } => {
                write!(f, "locale `{locale}` duplicates translation key `{key}`")
            }
            Self::MissingLocale { locale } => {
                write!(
                    f,
                    "locale `{locale}` is not registered in the translation runtime"
                )
            }
        }
    }
}

impl Error for I18nError {}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocaleRoutingStrategy {
    PathPrefix,
    Host,
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocaleUrlConfig {
    pub strategy: LocaleRoutingStrategy,
    pub canonical_host: String,
    pub host_map: HashMap<LocaleTag, String>,
}

impl LocaleUrlConfig {
    pub fn path_prefix(canonical_host: impl Into<String>) -> Result<Self, I18nError> {
        Ok(Self {
            strategy: LocaleRoutingStrategy::PathPrefix,
            canonical_host: require_non_empty("canonical_host", canonical_host.into())?,
            host_map: HashMap::new(),
        })
    }

    pub fn host_map(
        canonical_host: impl Into<String>,
        host_map: HashMap<LocaleTag, String>,
    ) -> Result<Self, I18nError> {
        Ok(Self {
            strategy: LocaleRoutingStrategy::Host,
            canonical_host: require_non_empty("canonical_host", canonical_host.into())?,
            host_map,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalizedUrls {
    pub canonical: String,
    pub alternate_hreflang: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocaleRouter {
    config: LocaleUrlConfig,
}

impl LocaleRouter {
    pub fn new(config: LocaleUrlConfig) -> Self {
        Self { config }
    }

    pub fn localized_path(&self, locale: &LocaleTag, path: &str) -> Result<String, I18nError> {
        let normalized = validate_route("path", path.to_string())?;
        match self.config.strategy {
            LocaleRoutingStrategy::PathPrefix => Ok(format!(
                "/{}/{}",
                locale.as_str().trim_matches('/'),
                normalized.trim_start_matches('/')
            )),
            LocaleRoutingStrategy::Host => Ok(normalized),
            LocaleRoutingStrategy::Mixed => Ok(format!(
                "/{}/{}",
                locale.as_str().trim_matches('/'),
                normalized.trim_start_matches('/')
            )),
        }
    }

    pub fn absolute_url(&self, locale: &LocaleTag, path: &str) -> Result<String, I18nError> {
        let host = match self.config.strategy {
            LocaleRoutingStrategy::Host | LocaleRoutingStrategy::Mixed => self
                .config
                .host_map
                .get(locale)
                .cloned()
                .unwrap_or_else(|| self.config.canonical_host.clone()),
            LocaleRoutingStrategy::PathPrefix => self.config.canonical_host.clone(),
        };

        let localized_path = self.localized_path(locale, path)?;
        Ok(format!("https://{host}{localized_path}"))
    }

    pub fn alternate_urls(
        &self,
        locales: &[LocaleTag],
        path: &str,
    ) -> Result<LocalizedUrls, I18nError> {
        let canonical_locale = locales.first().ok_or_else(|| I18nError::MissingLocale {
            locale: "canonical".to_string(),
        })?;
        let canonical = self.absolute_url(canonical_locale, path)?;
        let mut alternate_hreflang = BTreeMap::new();

        for locale in locales {
            alternate_hreflang.insert(locale.to_string(), self.absolute_url(locale, path)?);
        }

        Ok(LocalizedUrls {
            canonical,
            alternate_hreflang,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluralCategory {
    One,
    Other,
}

pub struct Formatter;

impl Formatter {
    pub fn plural_category(locale: &LocaleTag, value: i64) -> PluralCategory {
        match locale.as_str() {
            "fr-FR" => {
                if matches!(value, 0 | 1) {
                    PluralCategory::One
                } else {
                    PluralCategory::Other
                }
            }
            _ => {
                if value == 1 {
                    PluralCategory::One
                } else {
                    PluralCategory::Other
                }
            }
        }
    }

    pub fn format_number(locale: &LocaleTag, value: i64) -> String {
        let negative = value < 0;
        let mut digits = value.abs().to_string();
        let separator = if locale.as_str() == "fr-FR" { ' ' } else { ',' };
        let mut groups = Vec::new();
        while digits.len() > 3 {
            let remainder = digits.split_off(digits.len() - 3);
            groups.push(remainder);
        }
        groups.push(digits);
        groups.reverse();
        let rendered = groups.join(&separator.to_string());
        if negative {
            format!("-{rendered}")
        } else {
            rendered
        }
    }

    pub fn format_money(locale: &LocaleTag, currency: &CurrencyCode, minor_units: i64) -> String {
        let major = minor_units / 100;
        let cents = minor_units.abs() % 100;
        let number = Self::format_number(locale, major);
        let decimal_separator = if locale.as_str() == "fr-FR" { ',' } else { '.' };
        match locale.as_str() {
            "fr-FR" => format!("{number}{decimal_separator}{cents:02} {currency}"),
            _ => format!("{currency} {number}{decimal_separator}{cents:02}"),
        }
    }

    pub fn format_datetime(locale: &LocaleTag, unix_seconds: i64, timezone: &TimeZoneId) -> String {
        let label = match locale.as_str() {
            "fr-FR" => "heure locale",
            _ => "local time",
        };
        format!("{unix_seconds} ({timezone}, {label})")
    }
}

fn validate_token(field: &'static str, value: String) -> Result<String, I18nError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(I18nError::EmptyField { field });
    }

    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/'))
    {
        Ok(trimmed.to_string())
    } else {
        Err(I18nError::InvalidToken {
            field,
            value: trimmed.to_string(),
        })
    }
}

fn validate_route(field: &'static str, value: String) -> Result<String, I18nError> {
    let route = require_non_empty(field, value)?;
    if route.starts_with('/') {
        Ok(route)
    } else {
        Err(I18nError::InvalidRoute {
            field,
            value: route,
        })
    }
}

fn require_non_empty(field: &'static str, value: String) -> Result<String, I18nError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(I18nError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn locale(locale: &str) -> LocaleTag {
        LocaleTag::new(locale).unwrap()
    }

    fn key(key: &str) -> MessageKey {
        MessageKey::new(key).unwrap()
    }

    #[test]
    fn translation_runtime_uses_fallback_chain() {
        let runtime = TranslationRuntime::new(
            locale("en-GB"),
            vec![
                TranslationCatalog::new(
                    locale("en-GB"),
                    vec![
                        (key("checkout.title"), "Checkout".to_string()),
                        (key("events.book"), "Book now".to_string()),
                    ],
                )
                .unwrap(),
                TranslationCatalog::new(
                    locale("fr-FR"),
                    vec![(key("checkout.title"), "Paiement".to_string())],
                )
                .unwrap(),
            ],
        )
        .unwrap();

        let context = LocaleContext::new(
            locale("fr-FR"),
            vec![locale("en-GB")],
            CurrencyCode::new("EUR").unwrap(),
            TimeZoneId::new("Europe/Paris").unwrap(),
        );

        assert_eq!(
            runtime.translate(&context, &key("checkout.title")).unwrap(),
            "Paiement"
        );
        assert_eq!(
            runtime.translate(&context, &key("events.book")).unwrap(),
            "Book now"
        );
    }

    #[test]
    fn locale_router_builds_path_prefixed_and_alternate_urls() {
        let router = LocaleRouter::new(LocaleUrlConfig::path_prefix("www.example.com").unwrap());
        let localized = router
            .alternate_urls(
                &[locale("en-GB"), locale("fr-FR")],
                "/events/spring-tasting",
            )
            .unwrap();

        assert_eq!(
            localized.canonical,
            "https://www.example.com/en-GB/events/spring-tasting"
        );
        assert_eq!(
            localized.alternate_hreflang["fr-FR"],
            "https://www.example.com/fr-FR/events/spring-tasting"
        );
    }

    #[test]
    fn locale_router_supports_host_based_locale_urls() {
        let mut host_map = HashMap::new();
        host_map.insert(locale("en-GB"), "www.example.com".to_string());
        host_map.insert(locale("fr-FR"), "fr.example.com".to_string());
        let router =
            LocaleRouter::new(LocaleUrlConfig::host_map("www.example.com", host_map).unwrap());

        assert_eq!(
            router.absolute_url(&locale("fr-FR"), "/events").unwrap(),
            "https://fr.example.com/events"
        );
    }

    #[test]
    fn formatter_handles_numbers_money_and_plural_rules() {
        assert_eq!(
            Formatter::format_number(&locale("en-GB"), 1234567),
            "1,234,567"
        );
        assert_eq!(
            Formatter::format_number(&locale("fr-FR"), 1234567),
            "1 234 567"
        );
        assert_eq!(
            Formatter::format_money(&locale("en-GB"), &CurrencyCode::new("GBP").unwrap(), 12345),
            "GBP 123.45"
        );
        assert_eq!(
            Formatter::format_money(&locale("fr-FR"), &CurrencyCode::new("EUR").unwrap(), 12345),
            "123,45 EUR"
        );
        assert_eq!(
            Formatter::plural_category(&locale("en-GB"), 1),
            PluralCategory::One
        );
        assert_eq!(
            Formatter::plural_category(&locale("fr-FR"), 0),
            PluralCategory::One
        );
    }

    #[test]
    fn formatter_carries_timezone_in_datetime_output() {
        assert_eq!(
            Formatter::format_datetime(
                &locale("fr-FR"),
                1_710_000_000,
                &TimeZoneId::new("Europe/Paris").unwrap()
            ),
            "1710000000 (Europe/Paris, heure locale)"
        );
    }
}
