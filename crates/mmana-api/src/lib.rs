//! HTTP API for the MMANA engine.
//!
//! [`app`] builds the router so it can be served by `main.rs`, embedded in
//! another server, or exercised in tests without opening a socket.

pub mod error;
pub mod routes;

use axum::Router;
use axum::routing::{get, post};
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;

/// Guards against requests that would tie up the server.
#[derive(Debug, Clone)]
pub struct Limits {
    /// Cap applied to `options.maxPulses`.
    pub max_pulses: usize,
    pub max_sweep_points: usize,
    /// Directions (far field) or points (near field) per request.
    pub max_field_points: usize,
    /// Element editing: (operations + 1) × wires, which bounds the wires the
    /// edits can create (each edit copies at most one element).
    pub max_element_work: usize,
    /// Calculations one optimisation may run (requests may ask for fewer).
    pub max_optimize_calculations: usize,
    pub max_body_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_pulses: 4096,
            max_sweep_points: 401,
            max_field_points: 65_341,
            max_element_work: 1_000_000,
            max_optimize_calculations: 2000,
            max_body_bytes: 4 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct AppState {
    pub limits: Limits,
}

#[derive(OpenApi)]
#[openapi(
    info(title = "MMANA API", description = "MININEC3 antenna analysis (port of MMANA)."),
    paths(
        routes::health,
        routes::parse_maa,
        routes::export_maa,
        routes::calculate,
        routes::sweep,
        routes::far_field,
        routes::near_field,
        routes::edit_elements,
        routes::optimize,
        routes::edit_optimizer_setup,
        routes::apply_trial
    ),
    // reported by the desktop app while an optimisation runs
    components(schemas(mmana_core::optimize::Progress)),
    tags(
        (name = "calculations", description = "Antenna calculations"),
        (name = "files", description = "MMANA .maa file conversion"),
        (name = "editing", description = "Antenna editing"),
        (name = "optimization", description = "Optimisation of dimensions, loads, height and more"),
        (name = "system")
    )
)]
pub struct ApiDoc;

async fn openapi() -> axum::Json<utoipa::openapi::OpenApi> {
    axum::Json(ApiDoc::openapi())
}

pub fn app(state: AppState) -> Router {
    let body_limit = state.limits.max_body_bytes;
    Router::new()
        .route("/health", get(routes::health))
        .route("/openapi.json", get(openapi))
        .route("/v1/maa/parse", post(routes::parse_maa))
        .route("/v1/maa/export", post(routes::export_maa))
        .route("/v1/calculations", post(routes::calculate))
        .route("/v1/sweeps", post(routes::sweep))
        .route("/v1/far-field", post(routes::far_field))
        .route("/v1/near-field", post(routes::near_field))
        .route("/v1/elements", post(routes::edit_elements))
        .route("/v1/optimizations", post(routes::optimize))
        .route("/v1/optimizations/setup", post(routes::edit_optimizer_setup))
        .route("/v1/optimizations/apply", post(routes::apply_trial))
        .with_state(state)
        .layer(RequestBodyLimitLayer::new(body_limit))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}
