//! Tauri commands. They take the same request types as the HTTP API
//! (`mmana_core::contract`), so the UI talks to either back end with one set
//! of TypeScript types. Calculations run on the blocking pool.

use mmana_core::contract::{
    self, AppliedTrial, ApplyTrialRequest, CalculationRequest, ElementEditRequest, FarFieldRequest, FarFieldResponse,
    NearFieldRequest, NearFieldResponse, OptimizeRequest, OptimizerSetupRequest, ParsedProject, SweepRequest,
    SweepResponse,
};
use mmana_core::elements::ElementEdit;
use mmana_core::optimize::{OptimizeError, OptimizeOutcome, Progress, SetupEdit};
use mmana_core::{CalcError, CalcResult, Project, PulseLayout, maa};
use serde::Serialize;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tauri::ipc::Channel;

/// Same shape as the API's problem+json body.
#[derive(Debug, Serialize)]
pub struct CommandError {
    #[serde(rename = "type")]
    kind: &'static str,
    title: &'static str,
    detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<&'static str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    wires: Vec<usize>,
}

impl CommandError {
    fn new(kind: &'static str, title: &'static str, detail: String) -> Self {
        Self { kind, title, detail, code: None, wires: Vec::new() }
    }
}

impl From<CalcError> for CommandError {
    fn from(e: CalcError) -> Self {
        Self {
            code: Some(e.code()),
            wires: e.wires(),
            ..Self::new("calculation-failed", "The antenna cannot be calculated", e.to_string())
        }
    }
}

impl From<OptimizeError> for CommandError {
    fn from(e: OptimizeError) -> Self {
        Self::new("invalid-optimizer-setup", "The optimisation setup cannot run", e.to_string())
    }
}

fn io_error(path: &Path, e: impl std::fmt::Display) -> CommandError {
    CommandError::new("file-error", "File error", format!("{}: {e}", path.display()))
}

fn internal(e: impl std::fmt::Display) -> CommandError {
    CommandError::new("internal", "Internal error", e.to_string())
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, CommandError> + Send + 'static,
) -> Result<T, CommandError> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(internal)?
}

fn is_json(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("json"))
}

/// Opens a `.maa` file or a project saved as JSON.
#[tauri::command]
pub async fn open_project(path: String) -> Result<ParsedProject, CommandError> {
    blocking(move || {
        let path = Path::new(&path);
        let bytes = std::fs::read(path).map_err(|e| io_error(path, e))?;
        if is_json(path) {
            let project: Project = serde_json::from_slice(&bytes).map_err(|e| io_error(path, e))?;
            return Ok(ParsedProject { project, warnings: Vec::new() });
        }
        Ok(contract::parse_maa(&bytes))
    })
    .await
}

/// A file passed on the command line (`mmana-desktop antenna.maa`), opened at start-up.
#[derive(Serialize)]
pub struct StartupFile {
    path: String,
    name: String,
    parsed: ParsedProject,
}

#[tauri::command]
pub async fn startup_file() -> Result<Option<StartupFile>, CommandError> {
    let Some(path) = std::env::args().skip(1).find(|a| !a.starts_with('-')) else {
        return Ok(None);
    };
    let name = Path::new(&path).file_name().map_or(path.clone(), |n| n.to_string_lossy().into_owned());
    let parsed = open_project(path.clone()).await?;
    Ok(Some(StartupFile { path, name, parsed }))
}

/// Parses `.maa` contents (bundled samples, drag & drop).
#[tauri::command]
pub fn parse_maa(bytes: Vec<u8>) -> ParsedProject {
    contract::parse_maa(&bytes)
}

/// Writes a text export (CSV etc.) as UTF-8 with a BOM, which spreadsheet
/// applications need to detect the encoding.
#[tauri::command]
pub async fn save_text(path: String, contents: String) -> Result<(), CommandError> {
    blocking(move || {
        let path = Path::new(&path);
        let bytes = [b"\xEF\xBB\xBF".as_slice(), contents.as_bytes()].concat();
        std::fs::write(path, bytes).map_err(|e| io_error(path, e))
    })
    .await
}

/// Saves as `.maa` (Shift_JIS, readable by MMANA) or as project JSON, by extension.
#[tauri::command]
pub async fn save_project(path: String, project: Project) -> Result<(), CommandError> {
    blocking(move || {
        let path = Path::new(&path);
        let bytes = if is_json(path) {
            serde_json::to_vec_pretty(&project).map_err(internal)?
        } else {
            maa::encode(&maa::write(&project))
        };
        std::fs::write(path, bytes).map_err(|e| io_error(path, e))
    })
    .await
}

#[tauri::command]
pub async fn calculate(request: CalculationRequest) -> Result<CalcResult, CommandError> {
    blocking(move || Ok(contract::calculate(&request)?)).await
}

#[tauri::command]
pub async fn pulse_layout(request: CalculationRequest) -> Result<PulseLayout, CommandError> {
    blocking(move || Ok(contract::pulse_layout(&request)?)).await
}

#[tauri::command]
pub async fn sweep(request: SweepRequest) -> Result<SweepResponse, CommandError> {
    blocking(move || Ok(contract::sweep(&request)?)).await
}

#[tauri::command]
pub async fn far_field(request: FarFieldRequest) -> Result<FarFieldResponse, CommandError> {
    blocking(move || Ok(contract::far_field(&request)?)).await
}

#[tauri::command]
pub async fn near_field(request: NearFieldRequest) -> Result<NearFieldResponse, CommandError> {
    blocking(move || Ok(contract::near_field(&request)?)).await
}

/// Element editing; fast enough to answer on the IPC thread.
#[tauri::command]
pub fn edit_elements(request: ElementEditRequest) -> ElementEdit {
    contract::edit_elements(&request)
}

/// Stops the running optimisation (managed state).
#[derive(Default)]
pub struct Optimizer {
    cancel: Arc<AtomicBool>,
}

/// Optimisation. Calculations that improve the score are always reported;
/// the others at most every 40 ms, which is plenty for the log and keeps
/// small antennas (a calculation per millisecond) from flooding the UI.
#[tauri::command]
pub async fn optimize(
    state: tauri::State<'_, Optimizer>,
    request: OptimizeRequest,
    on_progress: Channel<Progress>,
) -> Result<OptimizeOutcome, CommandError> {
    let cancel = state.cancel.clone();
    cancel.store(false, Ordering::SeqCst);
    blocking(move || {
        let mut last: Option<Instant> = None;
        Ok(contract::optimize(&request, |p| {
            if p.improved || last.is_none_or(|t| t.elapsed() >= Duration::from_millis(40)) {
                let _ = on_progress.send(p.clone());
                last = Some(Instant::now());
            }
            !cancel.load(Ordering::SeqCst)
        })?)
    })
    .await
}

/// The running optimisation returns its best antenna so far.
#[tauri::command]
pub fn cancel_optimization(state: tauri::State<'_, Optimizer>) {
    state.cancel.store(true, Ordering::SeqCst);
}

/// Edits of the optimisation setup; fast enough for the IPC thread.
#[tauri::command]
pub fn edit_optimizer_setup(request: OptimizerSetupRequest) -> SetupEdit {
    contract::edit_optimizer_setup(&request)
}

#[tauri::command]
pub fn apply_trial(request: ApplyTrialRequest) -> Result<AppliedTrial, CommandError> {
    Ok(contract::apply_trial(&request)?)
}
