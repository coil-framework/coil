use coil::fission::core::env::RouteLocation;
use coil::fission::core::{JobRef, JobResource, JobSpec, ResourceKey, ShellRouteChanged};
use coil::fission::prelude::*;
use coil::{CoilPrincipal, CoilSessionState, protected_route_decision};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRequest;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionResponse {
    pub principal: Option<CoilPrincipal>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminRequest {
    pub section: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminSnapshot {
    pub open_orders: u64,
    pub upcoming_bookings: u64,
    pub published_products: u64,
    pub recent_activity: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminJobError {
    pub code: String,
    pub message: String,
}

#[derive(Debug)]
pub struct SessionJob;

impl JobSpec for SessionJob {
    type Request = SessionRequest;
    type Ok = SessionResponse;
    type Err = AdminJobError;

    const NAME: &'static str = "shoppr.session.read";
}

#[derive(Debug)]
pub struct AdminSnapshotJob;

impl JobSpec for AdminSnapshotJob {
    type Request = AdminRequest;
    type Ok = AdminSnapshot;
    type Err = AdminJobError;

    const NAME: &'static str = "shoppr.admin.snapshot";
}

pub const SESSION_JOB: JobRef<SessionJob> = JobRef::new(SessionJob::NAME);
pub const ADMIN_SNAPSHOT_JOB: JobRef<AdminSnapshotJob> = JobRef::new(AdminSnapshotJob::NAME);

#[derive(Clone, Debug)]
pub struct AdminState {
    pub current_path: String,
    pub session: CoilSessionState,
    pub snapshot: AsyncSnapshot<AdminSnapshot, AdminJobError>,
}

impl Default for AdminState {
    fn default() -> Self {
        Self {
            current_path: "/admin".to_string(),
            session: CoilSessionState::Loading,
            snapshot: AsyncSnapshot::waiting(),
        }
    }
}

impl GlobalState for AdminState {}

#[cfg(feature = "web")]
pub fn admin_web_app() -> WebApp<AdminState, AdminApp> {
    WebApp::<AdminState, _>::new(AdminApp)
        .mount("#fission-web-mount")
        .with_title("Shoppr operations")
        .with_route_handler(admin_route_changed)
        .with_startup_action(HydrateSession)
}

#[cfg(all(feature = "web", target_arch = "wasm32"))]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn run_admin_web() -> Result<(), wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    admin_web_app()
        .run()
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
}

#[fission_reducer(HydrateSession)]
pub fn hydrate_session(state: &mut AdminState, ctx: &mut ReducerContext<AdminState>) {
    state.session = CoilSessionState::Loading;
    let ok = ctx.effects.bind(SessionLoaded, session_loaded);
    let error = ctx.effects.bind(SessionFailed, session_failed);
    ctx.effects
        .app(SESSION_JOB, SessionRequest)
        .on_ok(ok)
        .on_err(error)
        .dispatch();
}

#[fission_reducer(SessionLoaded)]
pub fn session_loaded(state: &mut AdminState, ctx: &mut ReducerContext<AdminState>) {
    state.session = match ctx.input.job_ok(SESSION_JOB) {
        Some(SessionResponse {
            principal: Some(principal),
        }) => CoilSessionState::Authenticated(principal),
        Some(SessionResponse { principal: None }) => CoilSessionState::SignedOut,
        None => CoilSessionState::Failed,
    };
}

#[fission_reducer(SessionFailed)]
pub fn session_failed(state: &mut AdminState) {
    state.session = CoilSessionState::Failed;
}

#[fission_reducer(AdminLoaded)]
pub fn admin_loaded(state: &mut AdminState, ctx: &mut ReducerContext<AdminState>) {
    if let Some(snapshot) = ctx.input.job_ok(ADMIN_SNAPSHOT_JOB) {
        state.snapshot = AsyncSnapshot::with_data(AsyncConnectionState::Done, snapshot);
    }
}

#[fission_reducer(AdminFailed)]
pub fn admin_failed(state: &mut AdminState, ctx: &mut ReducerContext<AdminState>) {
    let error = ctx
        .input
        .job_err(ADMIN_SNAPSHOT_JOB)
        .unwrap_or(AdminJobError {
            code: "admin_unavailable".to_string(),
            message: "Shoppr operations could not be loaded".to_string(),
        });
    state.snapshot = AsyncSnapshot::with_error(AsyncConnectionState::Done, error);
}

pub fn admin_route_changed(
    state: &mut AdminState,
    action: ShellRouteChanged,
    _ctx: &mut ReducerContext<AdminState>,
) {
    state.current_path = action.location.logical_route();
}

#[derive(Clone, Copy)]
pub struct AdminApp;

impl From<AdminApp> for Widget {
    fn from(_: AdminApp) -> Self {
        let (ctx, view) = coil::fission::build::current::<AdminState>();
        ctx.register::<HydrateSession, _>(reduce_with!(hydrate_session));
        let location = RouteLocation::from_route(&view.state().current_path);
        let decision =
            protected_route_decision(&view.state().session, &location, "admin.dashboard.view");

        Router::<AdminState>::new()
            .with_path(view.state().current_path.clone())
            .route_component(
                "/admin",
                ProtectedRoute::new(decision.clone(), AdminDashboard)
                    .pending(AdminLoading)
                    .denied(AdminDenied),
            )
            .route_component(
                "/admin/catalog",
                ProtectedRoute::new(decision.clone(), AdminCatalog)
                    .pending(AdminLoading)
                    .denied(AdminDenied),
            )
            .route_component(
                "/admin/orders",
                ProtectedRoute::new(decision, AdminOrders)
                    .pending(AdminLoading)
                    .denied(AdminDenied),
            )
            .route_component("/sign-in", AdminSignIn)
            .into()
    }
}

#[derive(Clone, Copy)]
struct AdminDashboard;

impl From<AdminDashboard> for Widget {
    fn from(_: AdminDashboard) -> Self {
        let (ctx, view) = coil::fission::build::current::<AdminState>();
        let loaded = with_reducer!(ctx, AdminLoaded, admin_loaded);
        let failed = with_reducer!(ctx, AdminFailed, admin_failed);
        let request = AdminRequest {
            section: "dashboard".to_string(),
        };
        ctx.with_resources(|resources| {
            resources.job(
                JobResource::new(
                    ResourceKey::new("shoppr.admin.dashboard"),
                    ADMIN_SNAPSHOT_JOB,
                    request.clone(),
                )
                .deps(request)
                .on_ok(loaded)
                .on_err(failed),
            );
        });
        admin_shell("Command centre", admin_snapshot(&view.state().snapshot))
    }
}

#[derive(Clone, Copy)]
struct AdminCatalog;

impl From<AdminCatalog> for Widget {
    fn from(_: AdminCatalog) -> Self {
        admin_shell(
            "Catalogue",
            Text::new("Edit products, publication, inventory, and market visibility.").into(),
        )
    }
}

#[derive(Clone, Copy)]
struct AdminOrders;

impl From<AdminOrders> for Widget {
    fn from(_: AdminOrders) -> Self {
        admin_shell(
            "Orders",
            Text::new("Review payment state, fulfilment, refunds, and customer context.").into(),
        )
    }
}

fn admin_shell(title: &str, content: Widget) -> Widget {
    Container::new(Column {
        gap: Some(28.0),
        children: vec![
            Row {
                gap: Some(22.0),
                children: vec![
                    Text::new("SHOPPR OPERATIONS").weight(700).into(),
                    Link::to("Overview", "/admin").into(),
                    Link::to("Catalogue", "/admin/catalog").into(),
                    Link::to("Orders", "/admin/orders").into(),
                ],
                ..Default::default()
            }
            .into(),
            Divider::default().into(),
            Text::new(title).size(44.0).weight(700).into(),
            content,
        ],
        ..Default::default()
    })
    .padding_all(32.0)
    .into()
}

fn admin_snapshot(snapshot: &AsyncSnapshot<AdminSnapshot, AdminJobError>) -> Widget {
    match snapshot {
        AsyncSnapshot {
            data: Some(snapshot),
            ..
        } => Column {
            gap: Some(18.0),
            children: vec![
                metric("Open orders", snapshot.open_orders),
                Divider::default().into(),
                metric("Upcoming bookings", snapshot.upcoming_bookings),
                Divider::default().into(),
                metric("Published products", snapshot.published_products),
                Divider::default().into(),
                Timeline {
                    items: snapshot
                        .recent_activity
                        .iter()
                        .map(|activity| TimelineItem {
                            title: activity.clone(),
                            description: None,
                            timestamp: None,
                        })
                        .collect(),
                }
                .into(),
            ],
            ..Default::default()
        }
        .into(),
        AsyncSnapshot {
            error: Some(error), ..
        } => Text::new(error.message.clone()).into(),
        _ => Spinner {
            id: WidgetId::explicit("shoppr.admin.loading"),
            color: None,
            motion: Some(SpinnerMotion::Default),
        }
        .into(),
    }
}

fn metric(label: &str, value: u64) -> Widget {
    Row {
        gap: Some(20.0),
        children: vec![
            Text::new(label).size(18.0).into(),
            Spacer {
                flex_grow: 1.0,
                ..Default::default()
            }
            .into(),
            Text::new(value.to_string()).size(32.0).weight(700).into(),
        ],
        ..Default::default()
    }
    .into()
}

#[derive(Clone, Copy)]
struct AdminLoading;

impl From<AdminLoading> for Widget {
    fn from(_: AdminLoading) -> Self {
        Spinner {
            id: WidgetId::explicit("shoppr.admin.session.loading"),
            color: None,
            motion: Some(SpinnerMotion::Default),
        }
        .into()
    }
}

#[derive(Clone, Copy)]
struct AdminDenied;

impl From<AdminDenied> for Widget {
    fn from(_: AdminDenied) -> Self {
        Text::new("Your account does not have access to Shoppr operations").into()
    }
}

#[derive(Clone, Copy)]
struct AdminSignIn;

impl From<AdminSignIn> for Widget {
    fn from(_: AdminSignIn) -> Self {
        Text::new("Sign in to Shoppr operations")
            .size(38.0)
            .weight(700)
            .into()
    }
}
