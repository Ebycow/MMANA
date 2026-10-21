//! Request handlers. Each calculation runs on the blocking thread pool so
//! long solves never stall the async runtime.

use axum::Json;
use axum::body::Bytes;
use axum::extract::{FromRequest, Query, State};
use axum::http::header;
use axum::response::IntoResponse;
use mmana_core::contract::{
    self, AppliedTrial, ApplyTrialRequest, CalculationRequest, ElementEditRequest, FarFieldRequest, FarFieldResponse,
    NearFieldRequest, NearFieldResponse, OptimizeRequest, OptimizerSetupRequest, ParsedProject, SweepRequest,
    SweepResponse, axis_len,
};
use mmana_core::elements::ElementEdit;
use mmana_core::optimize::{OptimizeOutcome, SetupEdit};
use mmana_core::{CalcOptions, CalcResult, Project, PulseLayout, maa};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::error::{ApiError, Problem};
use crate::{AppState, Limits};

/// `Json` whose rejections become problem+json responses.
#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(ApiError))]
pub struct ApiJson<T>(pub T);

impl<T: Serialize> IntoResponse for ApiJson<T> {
    fn into_response(self) -> axum::response::Response {
        Json(self.0).into_response()
    }
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, ApiError> + Send + 'static) -> Result<T, ApiError> {
    tokio::task::spawn_blocking(f).await.map_err(|e| ApiError::Internal(e.to_string()))?
}

fn checked_options(mut opts: CalcOptions, limits: &Limits) -> CalcOptions {
    opts.max_pulses = opts.max_pulses.min(limits.max_pulses);
    opts
}

#[derive(Serialize, ToSchema)]
pub struct Health {
    pub status: &'static str,
    pub version: &'static str,
}

/// Liveness probe.
#[utoipa::path(get, path = "/health", tag = "system", responses((status = 200, body = Health)))]
pub async fn health() -> ApiJson<Health> {
    ApiJson(Health { status: "ok", version: env!("CARGO_PKG_VERSION") })
}

/// Reads an MMANA `.maa` file (Shift_JIS or UTF-8) into a project.
#[utoipa::path(
    post,
    path = "/v1/maa/parse",
    tag = "files",
    request_body(content = String, content_type = "application/octet-stream", description = "Contents of a .maa file"),
    responses((status = 200, body = ParsedProject), (status = 400, body = Problem))
)]
pub async fn parse_maa(body: Bytes) -> Result<ApiJson<ParsedProject>, ApiError> {
    if body.is_empty() {
        return Err(ApiError::BadRequest("empty body; send the .maa file contents".into()));
    }
    Ok(ApiJson(contract::parse_maa(&body)))
}

#[derive(Deserialize, IntoParams, Default)]
#[into_params(parameter_in = Query)]
pub struct ExportQuery {
    /// `shift_jis` (default, what MMANA reads) or `utf-8`.
    pub encoding: Option<String>,
}

/// Writes a project as an MMANA `.maa` file.
#[utoipa::path(
    post,
    path = "/v1/maa/export",
    tag = "files",
    params(ExportQuery),
    request_body = Project,
    responses((status = 200, content_type = "application/octet-stream", body = String), (status = 400, body = Problem))
)]
pub async fn export_maa(
    Query(q): Query<ExportQuery>,
    ApiJson(project): ApiJson<Project>,
) -> Result<impl IntoResponse, ApiError> {
    let text = maa::write(&project);
    let (bytes, charset) = match q.encoding.as_deref().unwrap_or("shift_jis") {
        "utf-8" | "utf8" => (text.into_bytes(), "utf-8"),
        "shift_jis" | "sjis" | "cp932" => (maa::encode(&text), "shift_jis"),
        other => return Err(ApiError::BadRequest(format!("unknown encoding '{other}'"))),
    };
    Ok(([(header::CONTENT_TYPE, format!("text/plain; charset={charset}"))], bytes))
}

/// Calculates impedance, SWR, gain, F/B, patterns and currents at one frequency.
#[utoipa::path(
    post,
    path = "/v1/calculations",
    tag = "calculations",
    request_body = CalculationRequest,
    responses((status = 200, body = CalcResult), (status = 400, body = Problem), (status = 422, body = Problem))
)]
pub async fn calculate(
    State(state): State<AppState>,
    ApiJson(req): ApiJson<CalculationRequest>,
) -> Result<ApiJson<CalcResult>, ApiError> {
    let req = CalculationRequest { options: checked_options(req.options, &state.limits), ..req };
    Ok(ApiJson(blocking(move || Ok(contract::calculate(&req)?)).await?))
}

/// The pulses the antenna is divided into, by wire, with the names that
/// address them and the positions of the sources and loads. Nothing is solved.
#[utoipa::path(
    post,
    path = "/v1/pulses",
    tag = "calculations",
    request_body = CalculationRequest,
    responses((status = 200, body = PulseLayout), (status = 400, body = Problem), (status = 422, body = Problem))
)]
pub async fn pulse_layout(
    State(state): State<AppState>,
    ApiJson(req): ApiJson<CalculationRequest>,
) -> Result<ApiJson<PulseLayout>, ApiError> {
    let req = CalculationRequest { options: checked_options(req.options, &state.limits), ..req };
    Ok(ApiJson(blocking(move || Ok(contract::pulse_layout(&req)?)).await?))
}

/// Frequency sweep: impedance, SWR, gain and F/B at evenly spaced frequencies.
#[utoipa::path(
    post,
    path = "/v1/sweeps",
    tag = "calculations",
    request_body = SweepRequest,
    responses((status = 200, body = SweepResponse), (status = 400, body = Problem), (status = 422, body = Problem))
)]
pub async fn sweep(
    State(state): State<AppState>,
    ApiJson(req): ApiJson<SweepRequest>,
) -> Result<ApiJson<SweepResponse>, ApiError> {
    if req.points == 0 || req.points > state.limits.max_sweep_points {
        return Err(ApiError::LimitExceeded(format!("points must be 1-{}", state.limits.max_sweep_points)));
    }
    if req.indices.as_ref().is_some_and(|i| i.len() > state.limits.max_sweep_points) {
        return Err(ApiError::LimitExceeded(format!("at most {} indices", state.limits.max_sweep_points)));
    }
    let req = SweepRequest { options: checked_options(req.options, &state.limits), ..req };
    Ok(ApiJson(blocking(move || Ok(contract::sweep(&req)?)).await?))
}

/// Far-field gain on an arbitrary zenith × azimuth grid.
#[utoipa::path(
    post,
    path = "/v1/far-field",
    tag = "calculations",
    request_body = FarFieldRequest,
    responses((status = 200, body = FarFieldResponse), (status = 400, body = Problem), (status = 422, body = Problem))
)]
pub async fn far_field(
    State(state): State<AppState>,
    ApiJson(req): ApiJson<FarFieldRequest>,
) -> Result<ApiJson<FarFieldResponse>, ApiError> {
    if axis_len(&req.zenith) * axis_len(&req.azimuth) > state.limits.max_field_points {
        return Err(ApiError::LimitExceeded(format!(
            "at most {} directions per request",
            state.limits.max_field_points
        )));
    }
    let req = FarFieldRequest { options: checked_options(req.options, &state.limits), ..req };
    Ok(ApiJson(blocking(move || Ok(contract::far_field(&req)?)).await?))
}

/// Near electric or magnetic field on a grid of points.
#[utoipa::path(
    post,
    path = "/v1/near-field",
    tag = "calculations",
    request_body = NearFieldRequest,
    responses((status = 200, body = NearFieldResponse), (status = 400, body = Problem), (status = 422, body = Problem))
)]
pub async fn near_field(
    State(state): State<AppState>,
    ApiJson(req): ApiJson<NearFieldRequest>,
) -> Result<ApiJson<NearFieldResponse>, ApiError> {
    let s = &req.spec;
    if axis_len(&s.x) * axis_len(&s.y) * axis_len(&s.z) > state.limits.max_field_points {
        return Err(ApiError::LimitExceeded(format!("at most {} points per request", state.limits.max_field_points)));
    }
    let req = NearFieldRequest { options: checked_options(req.options, &state.limits), ..req };
    Ok(ApiJson(blocking(move || Ok(contract::near_field(&req)?)).await?))
}

/// Element editing: groups the wires into elements, applies the edits in
/// order and returns the element table and the edited antenna.
#[utoipa::path(
    post,
    path = "/v1/elements",
    tag = "editing",
    request_body = ElementEditRequest,
    responses((status = 200, body = ElementEdit), (status = 400, body = Problem), (status = 422, body = Problem))
)]
pub async fn edit_elements(
    State(state): State<AppState>,
    ApiJson(req): ApiJson<ElementEditRequest>,
) -> Result<ApiJson<ElementEdit>, ApiError> {
    let work = (req.operations.len() + 1).saturating_mul(req.antenna.wires.len().max(8));
    if work > state.limits.max_element_work {
        return Err(ApiError::LimitExceeded(format!(
            "(operations + 1) × wires must be at most {}",
            state.limits.max_element_work
        )));
    }
    Ok(ApiJson(blocking(move || Ok(contract::edit_elements(&req))).await?))
}

/// Optimisation: varies the setup's variables one at a time while the score
/// of the goals rises, until a whole round improves nothing, and returns the
/// optimised antenna with the last trials. At most the server's calculation
/// limit (and `maxCalculations`) run; the best antenna so far is returned
/// then (`stop.kind` = `calculationLimit`).
#[utoipa::path(
    post,
    path = "/v1/optimizations",
    tag = "optimization",
    request_body = OptimizeRequest,
    responses((status = 200, body = OptimizeOutcome), (status = 400, body = Problem), (status = 422, body = Problem))
)]
pub async fn optimize(
    State(state): State<AppState>,
    ApiJson(req): ApiJson<OptimizeRequest>,
) -> Result<ApiJson<OptimizeOutcome>, ApiError> {
    let limit = state.limits.max_optimize_calculations;
    let req = OptimizeRequest {
        options: checked_options(req.options, &state.limits),
        max_calculations: Some(req.max_calculations.map_or(limit, |n| n.min(limit))),
        ..req
    };
    Ok(ApiJson(blocking(move || Ok(contract::optimize(&req, |_| true)?)).await?))
}

/// Edits of the optimisation setup (variables, "all elements", automatic
/// links), and the current values, element table and problems of the result.
#[utoipa::path(
    post,
    path = "/v1/optimizations/setup",
    tag = "optimization",
    request_body = OptimizerSetupRequest,
    responses((status = 200, body = SetupEdit), (status = 400, body = Problem), (status = 422, body = Problem))
)]
pub async fn edit_optimizer_setup(
    State(state): State<AppState>,
    ApiJson(req): ApiJson<OptimizerSetupRequest>,
) -> Result<ApiJson<SetupEdit>, ApiError> {
    // like element editing, every edit may walk all wires
    let work = (req.operations.len() + 1).saturating_mul(req.project.antenna.wires.len().max(8));
    if work > state.limits.max_element_work {
        return Err(ApiError::LimitExceeded(format!(
            "(operations + 1) × wires must be at most {}",
            state.limits.max_element_work
        )));
    }
    Ok(ApiJson(blocking(move || Ok(contract::edit_optimizer_setup(&req))).await?))
}

/// Writes the values of one optimisation trial back to the antenna.
#[utoipa::path(
    post,
    path = "/v1/optimizations/apply",
    tag = "optimization",
    request_body = ApplyTrialRequest,
    responses((status = 200, body = AppliedTrial), (status = 400, body = Problem), (status = 422, body = Problem))
)]
pub async fn apply_trial(ApiJson(req): ApiJson<ApplyTrialRequest>) -> Result<ApiJson<AppliedTrial>, ApiError> {
    Ok(ApiJson(contract::apply_trial(&req)?))
}
