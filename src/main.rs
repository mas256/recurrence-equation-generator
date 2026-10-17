use axum::{
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use recurrence_lab::{
    coverage, export,
    generator::{self, GeneratedProblem, Recipe},
    learning,
    selection::{self, SelectionRecord},
    verifier::{VerificationError, VerificationRecord, Verifier},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, VecDeque},
    fs,
    net::SocketAddr,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

#[derive(Clone)]
struct App {
    inner: Arc<Mutex<Store>>,
    verifier: Verifier,
    data_dir: PathBuf,
    limits: Limits,
    port: u16,
}
struct Store {
    jobs: HashMap<String, Job>,
    active: Option<String>,
    problems: HashMap<String, VerifiedProblem>,
    recent: VecDeque<SelectionRecord>,
}
struct Job {
    id: String,
    status: String,
    phase: String,
    message: String,
    started: Instant,
    elapsed_ms: Option<u128>,
    cancel: Arc<AtomicBool>,
    result: Option<VerifiedProblem>,
}
#[derive(Clone, Serialize, Deserialize)]
struct VerifiedProblem {
    problem: GeneratedProblem,
    verification: VerificationRecord,
}
#[derive(Clone, Serialize)]
struct Limits {
    candidate_limit: usize,
    search_timeout_seconds: u64,
    lean_timeout_seconds: u64,
    request_timeout_seconds: u64,
    lean_candidate_limit: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            candidate_limit: 200,
            search_timeout_seconds: 10,
            lean_timeout_seconds: 60,
            request_timeout_seconds: 120,
            lean_candidate_limit: 2,
        }
    }
}
#[derive(Deserialize)]
struct GenerateRequest {
    level: u8,
    #[serde(default)]
    family: Option<String>,
    seed: Option<u64>,
    #[serde(default = "standard_practice_mode")]
    practice_mode: String,
}
fn standard_practice_mode() -> String {
    "standard".into()
}
#[derive(Deserialize)]
struct ReplayRequest {
    recipe: Recipe,
}
enum Work {
    Generate(GenerateRequest),
    Replay(Box<Recipe>),
}
include!(concat!(env!("OUT_DIR"), "/web_assets.rs"));
type ApiResult<T> = Result<T, (StatusCode, Json<Value>)>;
fn error(code: StatusCode, message: impl Into<String>) -> (StatusCode, Json<Value>) {
    (code, Json(json!({"error":message.into()})))
}
fn authorize(headers: &HeaderMap, app: &App) -> ApiResult<()> {
    let hosts = [
        format!("127.0.0.1:{}", app.port),
        format!("localhost:{}", app.port),
    ];
    if let Some(host) = headers.get(header::HOST).and_then(|v| v.to_str().ok()) {
        if !hosts.iter().any(|h| h == host) {
            return Err(error(
                StatusCode::FORBIDDEN,
                "ローカルのURLから接続してください。",
            ));
        }
    }
    if let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
        if !hosts.iter().any(|h| origin == format!("http://{h}")) {
            return Err(error(
                StatusCode::FORBIDDEN,
                "異なるサイトからの要求は受け付けられません。",
            ));
        }
    }
    Ok(())
}
async fn status(State(app): State<App>) -> Json<Value> {
    let environment = app.verifier.status();
    let store = app.inner.lock().unwrap();
    Json(json!({"ready":environment.ready,"verification":environment,
        "families":generator::family_catalog(),"methods":learning::methods(),
        "coverage":coverage::catalog(),"variants":selection::catalog(),"selection_policy_version":selection::VERSION,"limits":app.limits,
        "verification_scope":{"native":"original_problem","routes":"finite_equation_checks"},
        "active_job":store.active,"history_count":store.recent.len(),
        "application_version":env!("CARGO_PKG_VERSION")}))
}
async fn generate(
    State(app): State<App>,
    headers: HeaderMap,
    Json(request): Json<GenerateRequest>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    authorize(&headers, &app)?;
    if !(1..=5).contains(&request.level) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "難易度はLv1〜Lv5を指定してください。",
        ));
    }
    if !["standard", "method_choice", "alternatives"].contains(&request.practice_mode.as_str()) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "演習形式を正しく指定してください。",
        ));
    }
    if let Some(family) = request.family.as_deref().filter(|x| *x != "mixed") {
        if !generator::family_catalog()
            .iter()
            .any(|f| f.id == family && f.levels.contains(&request.level))
        {
            return Err(error(
                StatusCode::BAD_REQUEST,
                "指定した型はこの難易度に対応していません。",
            ));
        }
    }
    start_job(app, Work::Generate(request)).await
}
async fn replay(
    State(app): State<App>,
    headers: HeaderMap,
    Json(request): Json<ReplayRequest>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    authorize(&headers, &app)?;
    generator::replay(&request.recipe).map_err(|e| error(StatusCode::BAD_REQUEST, e))?;
    start_job(app, Work::Replay(Box::new(request.recipe))).await
}
async fn start_job(app: App, work: Work) -> ApiResult<(StatusCode, Json<Value>)> {
    let environment = app.verifier.status();
    if !environment.ready {
        return Err(error(StatusCode::SERVICE_UNAVAILABLE, environment.message));
    }
    let id = uuid::Uuid::new_v4().to_string();
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut store = app.inner.lock().unwrap();
        if store.active.is_some() {
            return Err(error(
                StatusCode::CONFLICT,
                "処理中の問題があります。完了または中止までお待ちください。",
            ));
        }
        if store.jobs.len() >= 200 {
            let oldest = store
                .jobs
                .iter()
                .filter(|(_, j)| j.elapsed_ms.is_some())
                .min_by_key(|(_, j)| j.started)
                .map(|(id, _)| id.clone());
            if let Some(oldest) = oldest {
                store.jobs.remove(&oldest);
            }
        }
        store.active = Some(id.clone());
        store.jobs.insert(
            id.clone(),
            Job {
                id: id.clone(),
                status: "generating".into(),
                phase: "generating".into(),
                message: "問題の構成と解法を確認しています。".into(),
                started: Instant::now(),
                elapsed_ms: None,
                cancel: cancel.clone(),
                result: None,
            },
        );
    }
    let worker_id = id.clone();
    tokio::task::spawn_blocking(move || run_job(app, worker_id, work, cancel));
    Ok((
        StatusCode::ACCEPTED,
        Json(json!({"id":id,"status":"generating"})),
    ))
}
fn update_job(app: &App, id: &str, status: &str, phase: &str, message: &str) {
    if let Some(job) = app.inner.lock().unwrap().jobs.get_mut(id) {
        job.status = status.into();
        job.phase = phase.into();
        job.message = message.into();
    }
}
fn run_job(app: App, id: String, work: Work, cancel: Arc<AtomicBool>) {
    let started = Instant::now();
    let deadline = Duration::from_secs(app.limits.request_timeout_seconds);
    let mut last_error = "条件に合う候補を見つけられませんでした。".to_string();
    let fixed = match &work {
        Work::Replay(_) => true,
        Work::Generate(r) => r.seed.is_some(),
    };
    let mut searched = 0usize;
    let mut search_spent = Duration::ZERO;
    for proof_attempt in 0..app.limits.lean_candidate_limit {
        if cancel.load(Ordering::SeqCst) {
            finish(&app, &id, "cancelled", "生成を中止しました。", None);
            return;
        }
        if started.elapsed() >= deadline {
            finish(
                &app,
                &id,
                "timeout",
                "要求全体の時間上限に達しました。",
                None,
            );
            return;
        }
        let search_started = Instant::now();
        let remaining_budget = app.limits.candidate_limit.saturating_sub(searched);
        if remaining_budget == 0 {
            break;
        }
        let result = match &work {
            Work::Replay(recipe) => generator::replay(recipe),
            Work::Generate(r) => {
                let history: Vec<_> = app.inner.lock().unwrap().recent.iter().cloned().collect();
                let recent_ids: Vec<_> = history
                    .iter()
                    .rev()
                    .take(40)
                    .map(|h| h.id.clone())
                    .collect();
                let attempts_left = if fixed {
                    1
                } else {
                    app.limits.lean_candidate_limit - proof_attempt
                };
                let budget = remaining_budget.div_ceil(attempts_left);
                searched += budget;
                selection::generate(
                    r.level,
                    r.family.as_deref().filter(|f| *f != "mixed"),
                    r.seed.unwrap_or_else(rand::random),
                    &history,
                    &recent_ids,
                    &r.practice_mode,
                    budget,
                )
            }
        };
        let candidate = match result {
            Ok(problem) => Some(problem),
            Err(reason) => {
                last_error = reason;
                None
            }
        };
        search_spent += search_started.elapsed();
        let Some(problem) = candidate else {
            if cancel.load(Ordering::SeqCst) {
                finish(&app, &id, "cancelled", "生成を中止しました。", None);
            } else if started.elapsed() >= deadline
                || search_spent >= Duration::from_secs(app.limits.search_timeout_seconds)
            {
                finish(
                    &app,
                    &id,
                    "timeout",
                    "候補探索または要求全体の時間上限に達しました。",
                    None,
                );
            } else {
                finish(&app, &id, "failed", &last_error, None);
            }
            return;
        };
        if cancel.load(Ordering::SeqCst) {
            finish(&app, &id, "cancelled", "生成を中止しました。", None);
            return;
        }
        if started.elapsed() >= deadline
            || search_spent >= Duration::from_secs(app.limits.search_timeout_seconds)
        {
            finish(
                &app,
                &id,
                "timeout",
                "候補探索または要求全体の時間上限に達しました。",
                None,
            );
            return;
        }
        update_job(
            &app,
            &id,
            "verifying",
            "verifying",
            "Leanで初期条件・漸化式・一般項・一意性を検証しています。",
        );
        let remaining = deadline.saturating_sub(started.elapsed());
        let timeout = Duration::from_secs(app.limits.lean_timeout_seconds).min(remaining);
        match app.verifier.verify(&problem, cancel.clone(), timeout) {
            Ok(verification) => {
                if cancel.load(Ordering::SeqCst) {
                    finish(&app, &id, "cancelled", "生成を中止しました。", None);
                    return;
                }
                let verified = VerifiedProblem {
                    problem,
                    verification,
                };
                let path = app
                    .data_dir
                    .join("problems")
                    .join(format!("{}.json", verified.problem.id));
                match serde_json::to_vec_pretty(&verified)
                    .map_err(|e| e.to_string())
                    .and_then(|bytes| {
                        let temporary = path.with_extension("json.tmp");
                        fs::write(&temporary, bytes)
                            .and_then(|_| fs::rename(&temporary, &path))
                            .map_err(|e| e.to_string())
                    }) {
                    Ok(()) => finish(
                        &app,
                        &id,
                        "succeeded",
                        "検証済みの問題を作成しました。",
                        Some(verified),
                    ),
                    Err(e) => finish(
                        &app,
                        &id,
                        "failed",
                        &format!("検証記録を保存できませんでした: {e}"),
                        None,
                    ),
                }
                return;
            }
            Err(VerificationError::Cancelled) => {
                finish(&app, &id, "cancelled", "生成を中止しました。", None);
                return;
            }
            Err(VerificationError::Timeout) => {
                finish(
                    &app,
                    &id,
                    "timeout",
                    "Lean検証の時間上限に達しました。検証済みの問題は更新していません。",
                    None,
                );
                return;
            }
            Err(e) => {
                last_error = e.to_string();
                if fixed || proof_attempt + 1 >= app.limits.lean_candidate_limit {
                    break;
                }
                update_job(
                    &app,
                    &id,
                    "generating",
                    "generating",
                    "別の候補を作成しています。",
                );
            }
        }
    }
    finish(&app, &id, "failed", &last_error, None);
}
fn finish(app: &App, id: &str, status: &str, message: &str, result: Option<VerifiedProblem>) {
    let mut store = app.inner.lock().unwrap();
    let cancelled = store
        .jobs
        .get(id)
        .is_some_and(|j| j.cancel.load(Ordering::SeqCst));
    let status = if cancelled { "cancelled" } else { status };
    let result = if cancelled { None } else { result };
    if let Some(result) = &result {
        store.recent.push_back(SelectionRecord {
            id: result.problem.id.clone(),
            family: result.problem.recipe.family.clone(),
            variant: result
                .problem
                .variant
                .as_ref()
                .map(|v| v.id.clone())
                .unwrap_or_default(),
        });
        while store.recent.len() > 2_000 {
            store.recent.pop_front();
        }
        store
            .problems
            .insert(result.problem.id.clone(), result.clone());
        let history: Vec<_> = store.recent.iter().collect();
        if let Ok(bytes) = serde_json::to_vec(&history) {
            let temporary = app.data_dir.join("history.json.tmp");
            let _ = fs::write(&temporary, bytes)
                .and_then(|_| fs::rename(&temporary, app.data_dir.join("history.json")));
        }
    }
    if let Some(job) = store.jobs.get_mut(id) {
        job.status = status.into();
        job.phase = status.into();
        job.message = if cancelled {
            "生成を中止しました。".into()
        } else {
            message.into()
        };
        job.elapsed_ms = Some(job.started.elapsed().as_millis());
        job.result = result;
    }
    if store.active.as_deref() == Some(id) {
        store.active = None;
    }
}
async fn job(State(app): State<App>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    let store = app.inner.lock().unwrap();
    let job = store
        .jobs
        .get(&id)
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "要求が見つかりません。"))?;
    Ok(Json(
        json!({"id":job.id,"status":job.status,"phase":job.phase,"message":job.message,
        "elapsed_ms":job.elapsed_ms.unwrap_or_else(||job.started.elapsed().as_millis()),
        "problem":job.result.as_ref().map(|r|&r.problem),"verification":job.result.as_ref().map(|r|&r.verification)}),
    ))
}
async fn cancel(
    State(app): State<App>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<Json<Value>> {
    authorize(&headers, &app)?;
    let mut store = app.inner.lock().unwrap();
    let job = store
        .jobs
        .get_mut(&id)
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "要求が見つかりません。"))?;
    if job.elapsed_ms.is_none() {
        job.cancel.store(true, Ordering::SeqCst);
        job.phase = "cancelling".into();
        job.message = "実行中の処理を終了しています。".into();
    }
    Ok(Json(json!({"id":id,"status":job.status,"phase":job.phase})))
}
fn find_problem(app: &App, id: &str) -> ApiResult<VerifiedProblem> {
    app.inner
        .lock()
        .unwrap()
        .problems
        .get(id)
        .cloned()
        .ok_or_else(|| {
            error(
                StatusCode::NOT_FOUND,
                "この実行で検証済みの問題が見つかりません。保存したRecipeを再現してください。",
            )
        })
}
#[derive(Deserialize)]
struct TexQuery {
    #[serde(default)]
    solution: bool,
}
async fn tex(
    State(app): State<App>,
    Path(id): Path<String>,
    Query(query): Query<TexQuery>,
) -> ApiResult<Response> {
    let record = find_problem(&app, &id)?;
    let content = export::tex(&record.problem, query.solution);
    let filename = format!(
        "attachment; filename=recurrence-{}.tex",
        &record.problem.id[..record.problem.id.len().min(12)]
    );
    Ok((
        [
            (header::CONTENT_TYPE, "application/x-tex; charset=utf-8"),
            (header::CONTENT_DISPOSITION, filename.as_str()),
        ],
        content,
    )
        .into_response())
}
async fn recipe(State(app): State<App>, Path(id): Path<String>) -> ApiResult<Json<Recipe>> {
    Ok(Json(find_problem(&app, &id)?.problem.recipe))
}
async fn asset(Path(path): Path<String>) -> Response {
    serve_asset(&path)
}
async fn index() -> Response {
    serve_asset("index.html")
}
fn serve_asset(path: &str) -> Response {
    if path.split('/').any(|part| part == "..") || path.contains('\\') {
        return StatusCode::NOT_FOUND.into_response();
    }
    let target = PathBuf::from(path);
    let content_type = match target.extension().and_then(|s| s.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        "svg" => "image/svg+xml",
        "json" => "application/json",
        _ => "application/octet-stream",
    };
    match bundled_asset(path) {
        Some(bytes) => (
            [
                (header::CONTENT_TYPE, content_type),
                (header::CACHE_CONTROL, "no-cache"),
            ],
            bytes,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
fn decode_history(bytes: &[u8]) -> VecDeque<SelectionRecord> {
    let mut history = serde_json::from_slice::<VecDeque<SelectionRecord>>(bytes)
        .or_else(|_| {
            serde_json::from_slice::<VecDeque<(String, String)>>(bytes).map(|old| {
                old.into_iter()
                    .map(|(id, family)| SelectionRecord {
                        id,
                        family,
                        variant: String::new(),
                    })
                    .collect()
            })
        })
        .unwrap_or_default();
    while history.len() > 2_000 {
        history.pop_front();
    }
    history
}
fn usage() {
    println!("recurrence-lab [--port 3210] [--data-dir PATH] [--lean-dir PATH] [--check]\n  --candidate-limit N --search-timeout SECONDS --lean-timeout SECONDS\n  --request-timeout SECONDS --lean-candidates N\n\n初回準備: bash scripts/setup-lean.sh\n起動後: http://127.0.0.1:3210 をブラウザーで開く");
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::current_dir()
        .ok()
        .filter(|p| p.join("lean/lakefile.toml").is_file())
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(PathBuf::from))
                .filter(|p| p.join("lean/lakefile.toml").is_file())
        })
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    let mut port = 3210u16;
    let mut data_dir = root.join("data");
    let mut lean_dir = root.join("lean");
    let mut check = false;
    let mut limits = Limits::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--help" || arg == "-h" {
            usage();
            return Ok(());
        }
        if arg == "--check" {
            check = true;
            continue;
        }
        let value = args
            .next()
            .ok_or_else(|| format!("{arg} の値を指定してください。"))?;
        match arg.as_str() {
            "--port" => port = value.parse()?,
            "--data-dir" => data_dir = value.into(),
            "--lean-dir" => lean_dir = value.into(),
            "--candidate-limit" => limits.candidate_limit = value.parse()?,
            "--search-timeout" => limits.search_timeout_seconds = value.parse()?,
            "--lean-timeout" => limits.lean_timeout_seconds = value.parse()?,
            "--request-timeout" => limits.request_timeout_seconds = value.parse()?,
            "--lean-candidates" => limits.lean_candidate_limit = value.parse()?,
            _ => return Err(format!("不明なオプション: {arg}").into()),
        }
    }
    if port == 0
        || limits.candidate_limit == 0
        || limits.search_timeout_seconds == 0
        || limits.lean_timeout_seconds == 0
        || limits.request_timeout_seconds == 0
        || limits.lean_candidate_limit == 0
    {
        return Err("ポートと時間・候補数は正の値を指定してください。".into());
    }
    let verifier = Verifier::new(lean_dir);
    let environment = verifier.status();
    if check {
        println!("{}", serde_json::to_string_pretty(&environment)?);
        if !environment.ready {
            std::process::exit(1);
        }
        return Ok(());
    }
    fs::create_dir_all(data_dir.join("problems"))?;
    let recent = fs::read(data_dir.join("history.json"))
        .ok()
        .map(|bytes| decode_history(&bytes))
        .unwrap_or_default();
    let app = App {
        inner: Arc::new(Mutex::new(Store {
            jobs: HashMap::new(),
            active: None,
            problems: HashMap::new(),
            recent,
        })),
        verifier,
        data_dir,
        limits,
        port,
    };
    let router = Router::new()
        .route("/", get(index))
        .route("/api/status", get(status))
        .route("/api/generate", post(generate))
        .route("/api/replay", post(replay))
        .route("/api/jobs/{id}", get(job))
        .route("/api/jobs/{id}/cancel", post(cancel))
        .route("/api/problems/{id}/tex", get(tex))
        .route("/api/problems/{id}/recipe", get(recipe))
        .route("/{*path}", get(asset))
        .layer(DefaultBodyLimit::max(256 * 1024))
        .with_state(app.clone());
    let address = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(address).await?;
    println!(
        "漸化式ラボ: http://{address}\nLean: {}",
        environment.message
    );
    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            let _ = tokio::signal::ctrl_c().await;
            for job in app.inner.lock().unwrap().jobs.values() {
                job.cancel.store(true, Ordering::SeqCst);
            }
        })
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_migrates_old_family_records() {
        let history = decode_history(br#"[["old-id","affine"]]"#);
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].id, "old-id");
        assert_eq!(history[0].family, "affine");
        assert!(history[0].variant.is_empty());
    }
    #[test]
    fn history_retains_latest_two_thousand_branches() {
        let records: Vec<_> = (0..2_005)
            .map(|i| SelectionRecord {
                id: i.to_string(),
                family: "affine".into(),
                variant: "affine_default".into(),
            })
            .collect();
        let history = decode_history(&serde_json::to_vec(&records).unwrap());
        assert_eq!(history.len(), 2_000);
        assert_eq!(history.front().unwrap().id, "5");
        assert_eq!(history.back().unwrap().id, "2004");
    }
}
