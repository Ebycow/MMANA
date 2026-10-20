use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use mmana_api::{AppState, app};
use serde_json::{Value, json};
use tower::ServiceExt;

fn sample(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/../../testdata/maa/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

async fn send(req: Request<Body>) -> (StatusCode, String, Vec<u8>) {
    let res = app(AppState::default()).oneshot(req).await.unwrap();
    let status = res.status();
    let ctype = res.headers().get(header::CONTENT_TYPE).map(|v| v.to_str().unwrap().to_owned()).unwrap_or_default();
    let body = res.into_body().collect().await.unwrap().to_bytes().to_vec();
    (status, ctype, body)
}

async fn post_json(path: &str, body: Value) -> (StatusCode, String, Value) {
    let req = Request::post(path)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let (s, c, b) = send(req).await;
    (s, c, serde_json::from_slice(&b).unwrap_or(Value::Null))
}

async fn parse(name: &str) -> Value {
    let (status, _, body) = send(Request::post("/v1/maa/parse").body(Body::from(sample(name))).unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn health() {
    let (status, _, body) = send(Request::get("/health").body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["status"], "ok");
}

#[tokio::test]
async fn parse_maa_reads_shift_jis_sample() {
    let v = parse("3EL20.MAA").await;
    assert_eq!(v["project"]["antenna"]["name"], "３ele 20m (30mm/25mm/20mm Pipe)");
    assert_eq!(v["project"]["antenna"]["frequencyMhz"], 14.05);
    assert_eq!(v["project"]["environment"]["ground"], "real");
    assert_eq!(v["warnings"], json!([]));
}

#[tokio::test]
async fn calculation_matches_golden_result() {
    let parsed = parse("3EL20.MAA").await;
    let (status, _, r) = post_json("/v1/calculations", json!({ "project": parsed["project"] })).await;
    assert_eq!(status, StatusCode::OK, "{r}");
    let golden: Value = serde_json::from_str(
        &std::fs::read_to_string(format!("{}/../../testdata/golden/3EL20.json", env!("CARGO_MANIFEST_DIR"))).unwrap(),
    )
    .unwrap();
    let f32v = |v: &Value| v.as_f64().unwrap() as f32 as f64;
    assert_eq!(r["gainDbi"].as_f64().map(|v| v as f32 as f64), Some(f32v(&golden["result"]["gain"])));
    assert_eq!(r["fbDb"].as_f64().map(|v| v as f32 as f64), Some(f32v(&golden["result"]["fb"])));
    assert_eq!(r["pulseCount"], golden["pulses"]);
    assert_eq!(r["patterns"]["horizontal"]["total"].as_array().unwrap().len(), 360);
    assert_eq!(r["patterns"]["vertical"]["total"].as_array().unwrap().len(), 181);
}

#[tokio::test]
async fn export_round_trips_through_parse() {
    let parsed = parse("3EL20.MAA").await;
    let req = Request::post("/v1/maa/export?encoding=utf-8")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(parsed["project"].to_string()))
        .unwrap();
    let (status, ctype, text) = send(req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ctype, "text/plain; charset=utf-8");
    let (_, _, again) = send(Request::post("/v1/maa/parse").body(Body::from(text)).unwrap()).await;
    let again: Value = serde_json::from_slice(&again).unwrap();
    assert_eq!(again["project"], parsed["project"]);
}

#[tokio::test]
async fn sweep_returns_requested_points() {
    let parsed = parse("DP20.MAA").await;
    let (status, _, v) = post_json(
        "/v1/sweeps",
        json!({ "project": parsed["project"], "startMhz": 13.9, "stopMhz": 14.4, "points": 6,
                "options": { "patternResolution": "coarse" } }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    let pts = v["points"].as_array().unwrap();
    assert_eq!(pts.len(), 6);
    assert_eq!(pts[0]["frequencyMhz"], 13.9);
}

#[tokio::test]
async fn far_and_near_field() {
    let parsed = parse("DP20.MAA").await;
    let axis = |start: f64, step: f64, count: i32| json!({ "start": start, "step": step, "count": count });
    let (status, _, v) = post_json(
        "/v1/far-field",
        json!({ "project": parsed["project"], "zenith": axis(0.0, 30.0, 4), "azimuth": axis(0.0, 90.0, 4) }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["samples"].as_array().unwrap().len(), 16);

    let spec = json!({ "kind": "electric", "powerW": 100.0, "x": axis(1.0, 0.0, 1), "y": axis(0.0, 0.0, 1), "z": axis(20.0, 0.0, 1) });
    let (status, _, v) = post_json("/v1/near-field", json!({ "project": parsed["project"], "spec": spec })).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["samples"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn invalid_json_is_a_problem_response() {
    let (status, ctype, v) = post_json("/v1/calculations", json!({ "project": 1 })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(ctype, "application/problem+json");
    assert_eq!(v["type"], "invalid-request");
}

#[tokio::test]
async fn calculation_errors_are_422() {
    let mut parsed = parse("DP20.MAA").await;
    parsed["project"]["antenna"]["sources"] = json!([]);
    let (status, _, v) = post_json("/v1/calculations", json!({ "project": parsed["project"] })).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(v["type"], "calculation-failed");
    assert_eq!(v["detail"], "the antenna has no sources");
}

#[tokio::test]
async fn sweep_point_limit() {
    let parsed = parse("DP20.MAA").await;
    let (status, _, v) = post_json(
        "/v1/sweeps",
        json!({ "project": parsed["project"], "startMhz": 14.0, "stopMhz": 14.1, "points": 100000 }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(v["type"], "limit-exceeded");
}

#[tokio::test]
async fn element_editing() {
    let parsed = parse("3EL20.MAA").await;
    let antenna = &parsed["project"]["antenna"];
    let (status, _, v) = post_json(
        "/v1/elements",
        json!({ "antenna": antenna, "operations": [
            { "op": "setWidth", "row": 0, "value": 11.0 },
            { "op": "delete", "row": 1 },
            { "op": "setKind", "row": 2, "kind": "verticalSquare" },
        ] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["applied"], json!([true, false, true]));
    assert_eq!(v["base"], 1);
    let rows = v["elements"].as_array().unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0]["width"], 11.0);
    assert_eq!(rows[2]["kind"], "verticalSquare");
    assert_eq!(rows[2]["wireCount"], 4);
    assert_eq!(v["antenna"]["wires"].as_array().unwrap().len(), 6);
    assert_eq!(v["wireElements"], json!([1, 2, 0, 2, 2, 2]));
    assert_eq!(v["duplicate"], Value::Null);

    // (1000 edits + 1) × 1000 wires
    let mut big = antenna.clone();
    big["wires"] = Value::Array(vec![antenna["wires"][0].clone(); 1000]);
    let ops: Vec<Value> = (0..1000).map(|_| json!({ "op": "insert", "row": 0 })).collect();
    let (status, _, v) = post_json("/v1/elements", json!({ "antenna": big, "operations": ops })).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
    assert_eq!(v["type"], "limit-exceeded");
}

#[tokio::test]
async fn openapi_document_lists_every_route() {
    let (status, _, body) = send(Request::get("/openapi.json").body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    let v: Value = serde_json::from_slice(&body).unwrap();
    for path in [
        "/health",
        "/v1/maa/parse",
        "/v1/maa/export",
        "/v1/calculations",
        "/v1/sweeps",
        "/v1/far-field",
        "/v1/near-field",
        "/v1/elements",
        "/v1/optimizations",
        "/v1/optimizations/setup",
        "/v1/optimizations/apply",
    ] {
        assert!(v["paths"][path].is_object(), "missing {path}");
    }
    assert!(v["components"]["schemas"]["Project"].is_object());
    assert!(v["components"]["schemas"]["CalcResult"].is_object());
}

/// DP20 shortened to 9.2 m, far from resonance.
async fn short_dipole() -> Value {
    let mut project = parse("DP20.MAA").await["project"].clone();
    project["antenna"]["wires"][0]["start"]["y"] = json!(-4.6);
    project["antenna"]["wires"][0]["end"]["y"] = json!(4.6);
    project
}

/// Dipole length through wire 1's end and its mirror, scored on SWR.
async fn dipole_setup(project: &Value) -> Value {
    let (status, _, edit) = post_json(
        "/v1/optimizations/setup",
        json!({
            "project": project,
            "setup": { "weights": { "gain": 0, "fb": 0, "jx": 0, "swr": 100 } },
            "operations": [
                { "op": "setTarget", "index": 0, "target": { "kind": "wire", "wire": 0, "part": "y2" } },
                { "op": "linkSame", "index": 0 }
            ]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{edit}");
    assert_eq!(edit["applied"], json!([true, true]));
    assert_eq!(edit["errors"], json!([]));
    assert_eq!(edit["setup"]["variables"][1]["link"], json!({ "kind": "negated", "variable": 0 }));
    assert_eq!(edit["values"][0], json!(4.6));
    assert_eq!(edit["linkedValues"][1], json!(-4.6));
    edit["setup"].clone()
}

#[tokio::test]
async fn optimization_runs_and_applies_trials() {
    let project = &short_dipole().await;
    let setup = dipole_setup(project).await;
    let (status, _, out) =
        post_json("/v1/optimizations", json!({ "project": project, "setup": setup, "maxCalculations": 12 })).await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["stop"]["kind"], "calculationLimit");
    assert_eq!(out["calculations"], 12);
    let trials = out["trials"].as_array().unwrap();
    assert!(!trials.is_empty() && trials.len() <= 12);
    assert_eq!(trials[0]["horizontalPattern"].as_array().unwrap().len(), 180);
    let best = &out["best"]["values"];
    assert_eq!(out["project"]["antenna"]["wires"][0]["end"]["y"], best[0]);

    let values = trials.last().unwrap()["values"].clone();
    let (status, _, applied) =
        post_json("/v1/optimizations/apply", json!({ "project": project, "setup": setup, "values": values })).await;
    assert_eq!(status, StatusCode::OK, "{applied}");
    assert_eq!(applied["project"]["antenna"]["wires"][0]["start"]["y"], values[1]);
    assert_eq!(applied["frequencyMhz"], 14.05);
}

#[tokio::test]
async fn optimization_is_capped_by_the_server_limit() {
    let project = &short_dipole().await;
    let setup = dipole_setup(project).await;
    let mut state = AppState::default();
    state.limits.max_optimize_calculations = 5;
    let req = Request::post("/v1/optimizations")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "project": project, "setup": setup, "maxCalculations": 100 }).to_string()))
        .unwrap();
    let res = app(state).oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let out: Value = serde_json::from_slice(&res.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(out["calculations"], 5);
}

#[tokio::test]
async fn invalid_optimizer_setup_is_a_problem() {
    let parsed = parse("5EL20.MAA").await;
    let setup = json!({ "variables": [{ "target": { "kind": "height" }, "pitch": 1, "min": 0, "max": 100 }] });
    let (status, ctype, p) =
        post_json("/v1/optimizations", json!({ "project": parsed["project"], "setup": setup })).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(ctype, "application/problem+json");
    assert_eq!(p["type"], "invalid-optimizer-setup");
    assert!(p["detail"].as_str().unwrap().contains("free space"), "{p}");

    let (status, _, p) = post_json(
        "/v1/optimizations/apply",
        json!({ "project": parsed["project"], "setup": setup, "values": [1.0, 2.0] }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(p["detail"], "expected 1 values, got 2");
}
