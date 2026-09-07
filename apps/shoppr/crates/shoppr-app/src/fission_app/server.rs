use super::{ShopprState, StorefrontPage, StorefrontRoute};
use anyhow::{anyhow, Context, Result};
use coil::fission::i18n::TranslationBundle;
use coil::fission::server::{
    FissionServerApp, ServerJobRegistry, ServerRenderContext, WasmIsland, WebRouteMode,
};
use coil::{public_revalidation, SiteDefinition, SiteRegistry};
use coil_config::{Environment, PlatformConfig};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

pub fn shoppr_server_app(
    project_dir: impl Into<std::path::PathBuf>,
    config: &PlatformConfig,
    jobs: ServerJobRegistry,
) -> Result<FissionServerApp> {
    let project_dir = project_dir.into();
    let sites = Arc::new(site_registry(config)?);
    let public = || {
        WebRouteMode::Revalidated(public_revalidation(
            Duration::from_secs(300),
            ["shoppr-catalog"],
        ))
    };
    let locale_sites = Arc::clone(&sites);
    let mut app = FissionServerApp::new("Shoppr Townhouse")
        .project_dir(&project_dir)
        .jobs(jobs)
        .user_css(shoppr_fission::SHOPPR_CSS)
        .static_dir("/media", project_dir.join("theme/assets/images"))
        .default_locale("en-GB")
        .locale_resolver(move |ctx| {
            let host = ctx
                .request
                .headers
                .get("host")
                .ok_or_else(|| anyhow!("Shoppr requests require a Host header"))?;
            let locale = ctx.route_params.get("locale").map(String::as_str);
            let scope = locale_sites
                .resolve(host, locale, ctx.route_path, ctx.session.id())
                .context("Shoppr request locale resolution failed")?;
            Ok(scope.locale.as_str().into())
        })
        .route_widget_with_state(
            "/",
            "Shoppr Townhouse",
            Some("The current Shoppr edit.".to_string()),
            public(),
            StorefrontPage,
            state_loader(Arc::clone(&sites), StorefrontRoute::Home),
        )
        .route_widget_with_state(
            "/:locale",
            "Shoppr Townhouse",
            Some("The current Shoppr edit.".to_string()),
            public(),
            StorefrontPage,
            state_loader(Arc::clone(&sites), StorefrontRoute::Home),
        )
        .route_widget_with_state(
            "/:locale/shop",
            "New arrivals | Shoppr",
            Some("Shop the current Townhouse edit.".to_string()),
            public(),
            StorefrontPage,
            state_loader(Arc::clone(&sites), StorefrontRoute::Catalog),
        )
        .island(
            "/:locale/shop",
            WasmIsland::new(
                "shoppr-search",
                "/fission/islands/shoppr-search.wasm",
                "shoppr-search",
            )
            .entry("shoppr_fission::search_island_boot")
            .description(
                "Filter the current server-rendered catalogue without taking over the page.",
            ),
        )
        .island(
            "/:locale/shop",
            WasmIsland::new(
                "shoppr-cart",
                "/fission/islands/shoppr-cart.wasm",
                "shoppr-cart",
            )
            .entry("shoppr_fission::cart_island_boot")
            .description("Session cart editing and totals inside the catalogue page."),
        )
        .route_widget_with_state(
            "/:locale/shop/collections",
            "Seasonal edits | Shoppr",
            Some("Explore Shoppr collections.".to_string()),
            public(),
            StorefrontPage,
            state_loader(Arc::clone(&sites), StorefrontRoute::Collections),
        )
        .route_widget_with_state(
            "/:locale/shop/collections/:handle",
            "Collection | Shoppr",
            Some("A curated Shoppr collection.".to_string()),
            public(),
            StorefrontPage,
            state_loader(
                Arc::clone(&sites),
                StorefrontRoute::Collection(String::new()),
            ),
        )
        .route_widget_with_state(
            "/:locale/shop/products/:handle",
            "Product | Shoppr",
            Some("Shoppr product detail.".to_string()),
            public(),
            StorefrontPage,
            state_loader(Arc::clone(&sites), StorefrontRoute::Product(String::new())),
        )
        .island(
            "/:locale/shop/products/:handle",
            WasmIsland::new(
                "shoppr-cart",
                "/fission/islands/shoppr-cart.wasm",
                "shoppr-cart",
            )
            .entry("shoppr_fission::cart_island_boot")
            .description("Add the selected variant to the durable session cart."),
        )
        .route_widget_with_state(
            "/:locale/events",
            "Townhouse events | Shoppr",
            Some("Events and appointments at Shoppr spaces.".to_string()),
            public(),
            StorefrontPage,
            state_loader(Arc::clone(&sites), StorefrontRoute::Events),
        )
        .island(
            "/:locale/events",
            WasmIsland::new(
                "shoppr-booking",
                "/fission/islands/shoppr-booking.wasm",
                "shoppr-booking",
            )
            .entry("shoppr_fission::booking_island_boot")
            .description("Select and reserve an event session."),
        );
    for (locale, source) in [
        ("en-GB", include_str!("../../../../translations/en-GB.toml")),
        ("fr-FR", include_str!("../../../../translations/fr-FR.toml")),
        ("pl-PL", include_str!("../../../../translations/pl-PL.toml")),
    ] {
        app = app.translation_bundle(translation_bundle(locale, source)?);
    }
    Ok(app)
}

fn translation_bundle(locale: &str, source: &str) -> Result<TranslationBundle> {
    let document = source
        .parse::<toml::Value>()
        .with_context(|| format!("failed to parse the embedded {locale} Shoppr translations"))?;
    let mut messages = HashMap::new();
    flatten_translations("", &document, &mut messages);
    Ok(TranslationBundle {
        locale: locale.into(),
        messages,
    })
}

fn flatten_translations(prefix: &str, value: &toml::Value, messages: &mut HashMap<String, String>) {
    let Some(table) = value.as_table() else {
        return;
    };
    for (key, value) in table {
        let key = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        if let Some(message) = value.as_str() {
            messages.insert(key, message.to_string());
        } else {
            flatten_translations(&key, value, messages);
        }
    }
}

fn state_loader(
    sites: Arc<SiteRegistry>,
    route: StorefrontRoute,
) -> impl for<'a> Fn(&ServerRenderContext<'a>) -> Result<ShopprState> + Send + Sync + 'static {
    move |ctx| {
        let host = ctx
            .request
            .headers
            .get("host")
            .ok_or_else(|| anyhow!("Shoppr requests require a Host header"))?;
        let locale = ctx.route_params.get("locale").map(String::as_str);
        let route = match &route {
            StorefrontRoute::Collection(_) => StorefrontRoute::Collection(
                ctx.route_params
                    .get("handle")
                    .cloned()
                    .ok_or_else(|| anyhow!("collection route is missing its handle"))?,
            ),
            StorefrontRoute::Product(_) => StorefrontRoute::Product(
                ctx.route_params
                    .get("handle")
                    .cloned()
                    .ok_or_else(|| anyhow!("product route is missing its handle"))?,
            ),
            route => route.clone(),
        };
        let scope = sites
            .resolve(host, locale, ctx.route_path, ctx.session.id())
            .context("Shoppr request scope resolution failed")?;
        Ok(ShopprState::new(scope, route))
    }
}

fn site_registry(config: &PlatformConfig) -> Result<SiteRegistry> {
    let scheme = if matches!(config.app.environment, Environment::Development) {
        "http"
    } else {
        "https"
    };
    let sites = config.sites.iter().map(|site| {
        let market = site
            .default_locale
            .rsplit_once('-')
            .map(|(_, market)| market)
            .unwrap_or(site.default_locale.as_str());
        let mut definition = SiteDefinition::new(
            &site.id,
            format!("{scheme}://{}", site.canonical_host),
            market,
            &site.default_locale,
        )
        .with_host(&site.canonical_host);
        for host in &site.hosts {
            definition = definition.with_host(host);
        }
        for locale in &site.supported_locales {
            definition = definition.with_locale(locale);
        }
        definition
    });
    SiteRegistry::new(sites).context("invalid Shoppr site registry")
}
