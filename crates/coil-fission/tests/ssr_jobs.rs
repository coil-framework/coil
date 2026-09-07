use fission::core::{JobRef, JobResource, JobSpec, ResourceKey};
use fission::prelude::*;
use fission::server::{FissionServerApp, ServerJobRegistry, ServerRenderer, ServerRequest};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct CatalogRequest {
    site_id: String,
    locale: String,
}

#[derive(Debug)]
struct CatalogJob;

impl JobSpec for CatalogJob {
    type Request = CatalogRequest;
    type Ok = Vec<String>;
    type Err = String;

    const NAME: &'static str = "coil.test.catalog";
}

const CATALOG_JOB: JobRef<CatalogJob> = JobRef::new(CatalogJob::NAME);

#[derive(Clone, Debug)]
struct CatalogState {
    catalog: AsyncSnapshot<Vec<String>, String>,
}

impl Default for CatalogState {
    fn default() -> Self {
        Self {
            catalog: AsyncSnapshot::waiting(),
        }
    }
}

impl GlobalState for CatalogState {}

#[fission_reducer(CatalogLoaded)]
fn on_catalog_loaded(state: &mut CatalogState, ctx: &mut ReducerContext<CatalogState>) {
    if let Some(catalog) = ctx.input.job_ok(CATALOG_JOB) {
        state.catalog = AsyncSnapshot::with_data(AsyncConnectionState::Done, catalog);
    }
}

#[fission_reducer(CatalogFailed)]
fn on_catalog_failed(state: &mut CatalogState, ctx: &mut ReducerContext<CatalogState>) {
    let error = ctx
        .input
        .job_err(CATALOG_JOB)
        .unwrap_or_else(|| "catalog query failed".to_string());
    state.catalog = AsyncSnapshot::with_error(AsyncConnectionState::Done, error);
}

#[derive(Clone, Copy)]
struct CatalogPage;

impl From<CatalogPage> for Widget {
    fn from(_: CatalogPage) -> Self {
        let (ctx, view) = fission::build::current::<CatalogState>();
        let loaded = with_reducer!(ctx, CatalogLoaded, on_catalog_loaded);
        let failed = with_reducer!(ctx, CatalogFailed, on_catalog_failed);
        let request = CatalogRequest {
            site_id: "shoppr-uk".to_string(),
            locale: "en-GB".to_string(),
        };

        ctx.with_resources(|resources| {
            resources.job(
                JobResource::new(
                    ResourceKey::new("coil.catalog"),
                    CATALOG_JOB,
                    request.clone(),
                )
                .deps(request)
                .on_ok(loaded)
                .on_err(failed),
            );
        });

        match &view.state().catalog {
            AsyncSnapshot {
                data: Some(products),
                ..
            } => Column {
                children: products
                    .iter()
                    .cloned()
                    .map(Text::new)
                    .map(Into::into)
                    .collect(),
                ..Default::default()
            }
            .into(),
            AsyncSnapshot {
                error: Some(error), ..
            } => Text::new(error.clone()).into(),
            _ => Spinner {
                id: WidgetId::explicit("coil.catalog.loading"),
                color: None,
                motion: Some(SpinnerMotion::Default),
            }
            .into(),
        }
    }
}

#[test]
fn ssr_waits_for_the_catalog_job_then_renders_its_completion_state() {
    let calls = Arc::new(AtomicUsize::new(0));
    let handler_calls = Arc::clone(&calls);
    let app = FissionServerApp::new("Coil SSR proof")
        .jobs(
            ServerJobRegistry::new().register_job(CATALOG_JOB, move |request, _ctx| {
                handler_calls.fetch_add(1, Ordering::SeqCst);
                assert_eq!(request.site_id, "shoppr-uk");
                assert_eq!(request.locale, "en-GB");
                Ok(vec!["Linen overshirt".to_string()])
            }),
        )
        .server_route_widget::<CatalogState, _>("/", "Catalog", None, CatalogPage);
    let response = ServerRenderer::new(app)
        .handle(ServerRequest::get("/"))
        .unwrap();
    let html = response.body_string();

    assert_eq!(response.status, 200);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(html.contains("Linen overshirt"));
}
