//! Request / response types shared by every front end (HTTP API, Tauri
//! commands, CLI). Keeping them here gives the UI one contract regardless of
//! the transport, and the OpenAPI document generated from them is the source
//! of the TypeScript types.

use serde::{Deserialize, Serialize};

#[cfg(feature = "openapi")]
use utoipa::ToSchema;

use crate::elements::{ElementEdit, ElementOp};
use crate::optimize::{OptimizeError, OptimizeOutcome, OptimizerSetup, Progress, SetupEdit, SetupOp};
use crate::{
    Antenna, AxisRange, CalcError, CalcOptions, CalcResult, FarFieldSample, NearFieldSample, NearFieldSpec, Project,
    PulseLayout, SweepPoint, elements, maa, optimize,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct ParsedProject {
    pub project: Project,
    /// Non-fatal problems found while reading the file.
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct CalculationRequest {
    pub project: Project,
    #[serde(default)]
    pub options: CalcOptions,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SweepRequest {
    pub project: Project,
    #[serde(default)]
    pub options: CalcOptions,
    pub start_mhz: f64,
    pub stop_mhz: f64,
    pub points: usize,
    /// Evaluates only these points of the sweep (0-based), in this order, so
    /// a client can fill a long sweep in a few points at a time. All points
    /// when omitted.
    #[serde(default)]
    pub indices: Option<Vec<usize>>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct SweepResponse {
    pub points: Vec<SweepPoint>,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct FarFieldRequest {
    pub project: Project,
    #[serde(default)]
    pub options: CalcOptions,
    /// Zenith angles in degrees (0 = straight up).
    pub zenith: AxisRange,
    /// Azimuth angles in degrees (0 = +X).
    pub azimuth: AxisRange,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct FarFieldResponse {
    pub samples: Vec<FarFieldSample>,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct NearFieldRequest {
    pub project: Project,
    #[serde(default)]
    pub options: CalcOptions,
    /// Grid in absolute coordinates (the antenna height is included in Z when a ground is set).
    pub spec: NearFieldSpec,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct NearFieldResponse {
    pub samples: Vec<NearFieldSample>,
}

/// Element editing: the edits so far, replayed on the antenna as it was when
/// the editor opened.
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct ElementEditRequest {
    pub antenna: Antenna,
    #[serde(default)]
    pub operations: Vec<ElementOp>,
}

/// Optimisation. `options` are those the antenna is normally calculated
/// with: their frequency is where the search starts (the setup sets the
/// pattern resolution).
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct OptimizeRequest {
    pub project: Project,
    #[serde(default)]
    pub options: CalcOptions,
    pub setup: OptimizerSetup,
    /// Stop after this many calculations; the best antenna so far is returned.
    #[serde(default)]
    pub max_calculations: Option<usize>,
}

/// Edits of the optimisation setup, replayed on the setup as sent, and a
/// description of the result (current values, element table, problems).
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct OptimizerSetupRequest {
    pub project: Project,
    #[serde(default)]
    pub options: CalcOptions,
    #[serde(default)]
    pub setup: OptimizerSetup,
    #[serde(default)]
    pub operations: Vec<SetupOp>,
}

/// Writes the values of one trial back to the antenna.
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct ApplyTrialRequest {
    /// The antenna the optimisation started from (the trial's values are for
    /// its elements, which a later trial may have renumbered).
    pub project: Project,
    /// The options the optimisation started with.
    #[serde(default)]
    pub options: CalcOptions,
    pub setup: OptimizerSetup,
    /// One value per variable (`Trial::values`).
    pub values: Vec<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct AppliedTrial {
    pub project: Project,
    /// Calculation frequency (changed by a frequency variable).
    pub frequency_mhz: f64,
}

/// Number of samples an axis produces (a step of 0 means one sample).
pub fn axis_len(a: &AxisRange) -> usize {
    if a.step == 0.0 { 1 } else { a.count.max(1) as usize }
}

pub fn parse_maa(bytes: &[u8]) -> ParsedProject {
    let parsed = maa::parse_bytes(bytes);
    ParsedProject { project: parsed.project, warnings: parsed.warnings }
}

pub fn calculate(req: &CalculationRequest) -> Result<CalcResult, CalcError> {
    crate::calculate(&req.project, &req.options)
}

/// Where the pulses lie, without solving; for placing sources and loads.
pub fn pulse_layout(req: &CalculationRequest) -> Result<PulseLayout, CalcError> {
    crate::pulse_layout(&req.project, &req.options)
}

pub fn sweep(req: &SweepRequest) -> Result<SweepResponse, CalcError> {
    let Some(indices) = &req.indices else {
        return Ok(SweepResponse {
            points: crate::sweep(&req.project, &req.options, req.start_mhz, req.stop_mhz, req.points)?,
        });
    };
    let points = req.points.max(1);
    let frequencies = indices
        .iter()
        .map(|&index| {
            if index < points {
                Ok(crate::sweep_frequency(req.start_mhz, req.stop_mhz, points, index))
            } else {
                Err(CalcError::SweepPointOutOfRange { index, points })
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SweepResponse { points: crate::sweep_at(&req.project, &req.options, &frequencies)? })
}

pub fn far_field(req: &FarFieldRequest) -> Result<FarFieldResponse, CalcError> {
    let solved = crate::solve(&req.project, &req.options)?;
    let (z, a) = (req.zenith, req.azimuth);
    Ok(FarFieldResponse { samples: solved.far_field((z.start, z.step, z.count), (a.start, a.step, a.count)) })
}

pub fn near_field(req: &NearFieldRequest) -> Result<NearFieldResponse, CalcError> {
    Ok(NearFieldResponse { samples: crate::solve(&req.project, &req.options)?.near_field(&req.spec) })
}

pub fn edit_elements(req: &ElementEditRequest) -> ElementEdit {
    elements::edit_antenna(&req.antenna, &req.operations)
}

/// Runs the optimisation; `progress` sees every calculation and stops the
/// search by returning false.
pub fn optimize(
    req: &OptimizeRequest,
    progress: impl FnMut(&Progress) -> bool,
) -> Result<OptimizeOutcome, OptimizeError> {
    optimize::optimize(&req.project, &req.options, &req.setup, req.max_calculations, progress)
}

pub fn edit_optimizer_setup(req: &OptimizerSetupRequest) -> SetupEdit {
    optimize::edit_setup(&req.project, &req.options, &req.setup, &req.operations)
}

pub fn apply_trial(req: &ApplyTrialRequest) -> Result<AppliedTrial, OptimizeError> {
    let (project, frequency_mhz) = optimize::apply_trial(&req.project, &req.options, &req.setup, &req.values)?;
    Ok(AppliedTrial { project, frequency_mhz })
}
