//! Tauri commands. They take the same request types as the HTTP API
//! (`mmana_core::contract`), so the UI talks to either back end with one set
//! of TypeScript types. Calculations run on the blocking pool.

use mmana_core::contract::{
    self, CalculationRequest, ElementEditRequest, FarFieldRequest, FarFieldResponse, NearFieldRequest,
    NearFieldResponse, ParsedProject, SweepRequest, SweepResponse,
};
use mmana_core::elements::ElementEdit;
use mmana_core::{CalcError, CalcResult, Project, maa};
use serde::Serialize;
use std::path::Path;

/// Same shape as the API's problem+json body.
#[derive(Debug, Serialize)]
pub struct CommandError {
    #[serde(rename = "type")]
    kind: &'static str,
    title: &'static str,
    detail: String,
}

impl From<CalcError> for CommandError {
    fn from(e: CalcError) -> Self {
        Self { kind: "calculation-failed", title: "The antenna cannot be calculated", detail: e.to_string() }
    }
}

fn io_error(path: &Path, e: impl std::fmt::Display) -> CommandError {
    CommandError { kind: "file-error", title: "File error", detail: format!("{}: {e}", path.display()) }
}

fn internal(e: impl std::fmt::Display) -> CommandError {
    CommandError { kind: "internal", title: "Internal error", detail: e.to_string() }
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
