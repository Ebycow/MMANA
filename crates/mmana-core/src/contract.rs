//! Request / response types shared by every front end (HTTP API, Tauri
//! commands, CLI). Keeping them here gives the UI one contract regardless of
//! the transport, and the OpenAPI document generated from them is the source
//! of the TypeScript types.

use serde::{Deserialize, Serialize};

#[cfg(feature = "openapi")]
use utoipa::ToSchema;

use crate::elements::{ElementEdit, ElementOp};
use crate::{
    Antenna, AxisRange, CalcError, CalcOptions, CalcResult, FarFieldSample, NearFieldSample, NearFieldSpec, Project,
    SweepPoint, elements, maa,
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

pub fn sweep(req: &SweepRequest) -> Result<SweepResponse, CalcError> {
    Ok(SweepResponse { points: crate::sweep(&req.project, &req.options, req.start_mhz, req.stop_mhz, req.points)? })
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
