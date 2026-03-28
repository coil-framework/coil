use std::collections::HashMap;
use std::fs;

use crate::{
    CurrencyCode, Formatter, LocaleContext, LocaleRouter, LocaleTag, LocaleUrlConfig, MessageKey,
    PluralCategory, TimeZoneId, TranslationCatalog, TranslationRuntime,
};

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
fn translation_runtime_resolves_merged_messages_for_render_models() {
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
    let resolved = runtime.resolved_messages(&context);

    assert_eq!(
        resolved.get(&key("checkout.title")).map(String::as_str),
        Some("Paiement")
    );
    assert_eq!(
        resolved.get(&key("events.book")).map(String::as_str),
        Some("Book now")
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
    let router = LocaleRouter::new(LocaleUrlConfig::host_map("www.example.com", host_map).unwrap());

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
fn translation_catalog_loads_nested_toml_messages() {
    let catalog = TranslationCatalog::from_toml_str(
        locale("fr-FR"),
        r#"
[home]
title = "Accueil"

[nav]
checkout = "Paiement"
"#,
    )
    .unwrap();

    assert_eq!(catalog.translate(&key("home.title")), Some("Accueil"));
    assert_eq!(catalog.translate(&key("nav.checkout")), Some("Paiement"));
}

#[test]
fn translation_catalog_reads_toml_file() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("fr-FR.toml");
    fs::write(
        &path,
        r#"
[checkout]
title = "Paiement"
"#,
    )
    .unwrap();

    let catalog = TranslationCatalog::from_toml_file(locale("fr-FR"), &path).unwrap();

    assert_eq!(catalog.translate(&key("checkout.title")), Some("Paiement"));
}

#[test]
fn translation_catalog_rejects_non_string_values() {
    let error = TranslationCatalog::from_toml_str(
        locale("en-GB"),
        r#"
[checkout]
attempts = 3
"#,
    )
    .unwrap_err();

    assert!(error.to_string().contains("checkout.attempts"));
}
