use std::collections::BTreeMap;

use davenda_i18n::{LocaleRouter, LocaleTag, LocaleUrlConfig};

use crate::{
    HeadMetadata, OpenGraphData, OpenGraphType, RobotsDirective, SitemapChangeFrequency,
    SitemapDocument, SitemapEntry, SitemapImage, event_node, page_node, product_node,
};

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
