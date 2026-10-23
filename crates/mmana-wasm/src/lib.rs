//! WebAssembly bindings for the browser build. Requests and responses are the
//! JSON of `mmana_core::contract`, as over HTTP and Tauri, so the UI keeps one
//! set of TypeScript types. One `call` entry point dispatches by name: a new
//! contract function needs one line in `dispatch`, nothing on the JS side.
//!
//! The UI runs this in a Web Worker (single-threaded: `par` falls back to
//! plain loops on wasm32).

use mmana_core::contract;
use mmana_core::optimize::OptimizeError;
use mmana_core::{CalcError, Project, maa};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// Same shape as the API's problem+json body and the Tauri `CommandError`.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Problem {
    #[serde(rename = "type")]
    pub kind: String,
    pub title: String,
    pub detail: String,
}

impl Problem {
    fn new(kind: &str, title: &str, detail: impl ToString) -> Self {
        Self { kind: kind.into(), title: title.into(), detail: detail.to_string() }
    }
}

impl From<CalcError> for Problem {
    fn from(e: CalcError) -> Self {
        Self::new("calculation-failed", "The antenna cannot be calculated", e)
    }
}

impl From<OptimizeError> for Problem {
    fn from(e: OptimizeError) -> Self {
        Self::new("invalid-optimizer-setup", "The optimisation setup cannot run", e)
    }
}

impl From<std::convert::Infallible> for Problem {
    fn from(e: std::convert::Infallible) -> Self {
        match e {}
    }
}

fn invalid_request(e: serde_json::Error) -> Problem {
    Problem::new("invalid-request", "Invalid request", e)
}

fn internal(e: impl ToString) -> Problem {
    Problem::new("internal", "Internal error", e)
}

/// Parses the request, runs `f` and serialises its result.
fn json<Req: DeserializeOwned, Res: Serialize, E: Into<Problem>>(
    request: &str,
    f: impl FnOnce(&Req) -> Result<Res, E>,
) -> Result<String, Problem> {
    let req = serde_json::from_str(request).map_err(invalid_request)?;
    serde_json::to_string(&f(&req).map_err(Into::into)?).map_err(internal)
}

/// Runs the contract function `op` (camelCase, as the TypeScript `Transport`
/// names it) on a JSON request.
pub fn dispatch(op: &str, request: &str) -> Result<String, Problem> {
    use std::convert::Infallible;
    match op {
        "calculate" => json(request, contract::calculate),
        "pulseLayout" => json(request, contract::pulse_layout),
        "sweep" => json(request, contract::sweep),
        "farField" => json(request, contract::far_field),
        "nearField" => json(request, contract::near_field),
        "editElements" => json(request, |r| Ok::<_, Infallible>(contract::edit_elements(r))),
        "editOptimizerSetup" => json(request, |r| Ok::<_, Infallible>(contract::edit_optimizer_setup(r))),
        "applyTrial" => json(request, contract::apply_trial),
        _ => Err(Problem::new("invalid-request", "Unknown operation", op)),
    }
}

/// Runs an optimisation; `progress` sees every calculation and stops the
/// search by returning false.
pub fn run_optimize(request: &str, mut progress: impl FnMut(&str) -> bool) -> Result<String, Problem> {
    json(request, |r| contract::optimize(r, |p| serde_json::to_string(p).is_ok_and(|p| progress(&p))))
}

/// Saves as `.maa` (Shift_JIS, readable by MMANA).
pub fn export_maa_bytes(project: &str) -> Result<Vec<u8>, Problem> {
    let project: Project = serde_json::from_str(project).map_err(invalid_request)?;
    Ok(maa::encode(&maa::write(&project)))
}

/// A problem crosses to JS as its JSON text, so the worker can tell it from a
/// trap (a `WebAssembly.RuntimeError`).
fn to_js(p: Problem) -> JsValue {
    JsValue::from_str(&serde_json::to_string(&p).unwrap_or_default())
}

#[wasm_bindgen]
pub fn call(op: &str, request: &str) -> Result<String, JsValue> {
    dispatch(op, request).map_err(to_js)
}

/// Reads a `.maa` file; never fails (problems become warnings).
#[wasm_bindgen(js_name = parseMaa)]
pub fn parse_maa(bytes: &[u8]) -> Result<String, JsValue> {
    serde_json::to_string(&contract::parse_maa(bytes)).map_err(|e| to_js(internal(e)))
}

#[wasm_bindgen(js_name = exportMaa)]
pub fn export_maa(project: &str) -> Result<Vec<u8>, JsValue> {
    export_maa_bytes(project).map_err(to_js)
}

/// `progress(json)` is called for every calculation; a falsy return (or a
/// throw) stops the search, which then returns its best antenna so far.
#[wasm_bindgen]
pub fn optimize(request: &str, progress: &js_sys::Function) -> Result<String, JsValue> {
    run_optimize(request, |p| {
        progress.call1(&JsValue::NULL, &JsValue::from_str(p)).is_ok_and(|keep_going| keep_going.is_truthy())
    })
    .map_err(to_js)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json as j};

    fn dipole() -> Value {
        let parsed = contract::parse_maa(&std::fs::read("../../testdata/maa/DP20.MAA").unwrap());
        serde_json::to_value(parsed.project).unwrap()
    }

    #[test]
    fn dispatch_matches_the_contract() {
        let req = j!({ "project": dipole() }).to_string();
        let direct = contract::calculate(&serde_json::from_str(&req).unwrap()).unwrap();
        assert_eq!(dispatch("calculate", &req).unwrap(), serde_json::to_string(&direct).unwrap());
    }

    #[test]
    fn errors_are_problems() {
        assert_eq!(dispatch("calculate", "{").unwrap_err().kind, "invalid-request");
        assert_eq!(dispatch("nope", "{}").unwrap_err().title, "Unknown operation");
        let mut project = dipole();
        project["antenna"]["wires"] = j!([]);
        let req = j!({ "project": project }).to_string();
        assert_eq!(dispatch("calculate", &req).unwrap_err().kind, "calculation-failed");
    }

    #[test]
    fn optimize_reports_progress_and_stops_when_told() {
        let project = dipole();
        let edit: Value = serde_json::from_str(
            &dispatch(
                "editOptimizerSetup",
                &j!({
                    "project": project,
                    "setup": { "weights": { "gain": 0, "fb": 0, "jx": 0, "swr": 100 } },
                    "operations": [
                        { "op": "setTarget", "index": 0, "target": { "kind": "wire", "wire": 0, "part": "y2" } },
                        { "op": "linkSame", "index": 0 }
                    ]
                })
                .to_string(),
            )
            .unwrap(),
        )
        .unwrap();
        let req = j!({ "project": project, "setup": edit["setup"], "maxCalculations": 50 }).to_string();
        let mut seen = 0;
        let out: Value = serde_json::from_str(
            &run_optimize(&req, |p| {
                seen += 1;
                assert!(serde_json::from_str::<Value>(p).unwrap()["calculation"].is_u64());
                seen < 3
            })
            .unwrap(),
        )
        .unwrap();
        assert_eq!(seen, 3);
        assert_eq!(out["stop"]["kind"], "cancelled");
        assert_eq!(out["calculations"], 3);
    }

    #[test]
    fn maa_round_trip() {
        let bytes = export_maa_bytes(&dipole().to_string()).unwrap();
        assert_eq!(serde_json::to_value(contract::parse_maa(&bytes).project).unwrap(), dipole());
    }
}
