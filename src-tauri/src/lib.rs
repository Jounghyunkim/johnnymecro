mod agent;
mod browser;
mod engine;
mod ollama;
mod recorder;
mod scenario;
mod vault;

use browser::{Browser, CdpEvent};
use scenario::*;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::mpsc::UnboundedReceiver;

pub struct AppState {
    pub data_dir: PathBuf,
    pub settings: Mutex<Settings>,
    pub browser: Mutex<Option<Arc<Browser>>>,
    pub chrome: Mutex<Option<std::process::Child>>,
    pub recorder: Mutex<recorder::Recorder>,
    pub running: AtomicBool,
    pub cancel: Arc<AtomicBool>,
    pub confirms: Mutex<HashMap<String, tokio::sync::oneshot::Sender<bool>>>,
    pub cred_prompts: Mutex<HashMap<String, tokio::sync::oneshot::Sender<Option<vault::CredAnswer>>>>,
}

impl AppState {
    fn scenarios_dir(&self) -> PathBuf {
        self.data_dir.join("scenarios")
    }
    fn settings_path(&self) -> PathBuf {
        self.data_dir.join("settings.json")
    }
    pub fn browser(&self) -> Result<Arc<Browser>, String> {
        match self.browser.lock().unwrap().as_ref() {
            Some(b) if b.is_alive() => Ok(b.clone()),
            _ => Err("브라우저에 연결되어 있지 않습니다. 먼저 [크롬 실행/연결]을 누르세요".into()),
        }
    }
}

// ---------- browser ----------

fn browser_status_value(state: &AppState) -> Value {
    let b = state.browser.lock().unwrap();
    match b.as_ref() {
        Some(b) if b.is_alive() => json!({"connected": true, "port": b.port, "pages": b.pages()}),
        _ => json!({"connected": false, "pages": []}),
    }
}

fn emit_browser_state(app: &AppHandle) {
    let state = app.state::<AppState>();
    let _ = app.emit("browser://state", browser_status_value(&state));
}

async fn event_loop(app: AppHandle, browser: Arc<Browser>, mut rx: UnboundedReceiver<CdpEvent>) {
    while let Some(ev) = rx.recv().await {
        match ev.method.as_str() {
            "Target.targetCreated" => {
                browser.maybe_attach(&ev.params["targetInfo"]).await;
                emit_browser_state(&app);
            }
            "Target.targetInfoChanged" => {
                browser.on_target_info(&ev.params["targetInfo"]);
                emit_browser_state(&app);
            }
            "Target.targetDestroyed" => {
                browser.on_target_gone(ev.params["targetId"].as_str(), None);
                emit_browser_state(&app);
            }
            "Target.detachedFromTarget" => {
                browser.on_target_gone(None, ev.params["sessionId"].as_str());
                emit_browser_state(&app);
            }
            "Runtime.bindingCalled" => {
                if ev.params["name"].as_str() == Some(browser::BINDING) {
                    if let (Some(session), Some(payload)) = (ev.session_id.as_deref(), ev.params["payload"].as_str()) {
                        recorder::on_page_event(&app, &browser, session, payload);
                    }
                }
            }
            "Page.frameNavigated" => {
                if ev.params["frame"]["parentId"].is_null() {
                    if let Some(session) = ev.session_id.as_deref() {
                        recorder::on_navigated(&app, &browser, session, ev.params["frame"]["url"].as_str().unwrap_or(""));
                    }
                }
            }
            "__closed" => break,
            _ => {}
        }
    }
    {
        let state = app.state::<AppState>();
        let mut slot = state.browser.lock().unwrap();
        if slot.as_ref().map(|b| Arc::ptr_eq(b, &browser)).unwrap_or(false) {
            *slot = None;
        }
        state.recorder.lock().unwrap().active = false;
    }
    engine::log(&app, "warn", "브라우저 연결이 종료되었습니다");
    let _ = app.emit("record://state", json!({"recording": false}));
    emit_browser_state(&app);
}

async fn do_connect(app: &AppHandle, port: u16) -> Result<(), String> {
    let (b, rx) = Browser::connect(port).await?;
    b.attach_existing().await?;
    {
        let state = app.state::<AppState>();
        let mut slot = state.browser.lock().unwrap();
        if let Some(old) = slot.take() {
            old.close();
        }
        *slot = Some(b.clone());
    }
    tauri::async_runtime::spawn(event_loop(app.clone(), b, rx));
    emit_browser_state(app);
    Ok(())
}

/// Connect to an already-running Chrome on the debug port, or launch one.
#[tauri::command]
async fn browser_launch(app: AppHandle, state: State<'_, AppState>) -> Result<Value, String> {
    if state.browser().is_ok() {
        return Ok(browser_status_value(&state));
    }
    let settings = state.settings.lock().unwrap().clone();
    let port = settings.debug_port;
    if do_connect(&app, port).await.is_ok() {
        engine::log(&app, "success", format!("실행 중인 크롬(포트 {port})에 연결했습니다"));
        return Ok(browser_status_value(&state));
    }
    let exe = browser::find_chrome(&settings.chrome_path).ok_or("chrome.exe를 찾을 수 없습니다. 설정에서 크롬 경로를 지정하세요")?;
    let profile = if settings.user_data_dir.trim().is_empty() { state.data_dir.join("chrome-profile") } else { PathBuf::from(settings.user_data_dir.trim()) };
    std::fs::create_dir_all(&profile).map_err(|e| e.to_string())?;
    let child = std::process::Command::new(&exe)
        .arg(format!("--remote-debugging-port={port}"))
        .arg(format!("--user-data-dir={}", profile.display()))
        .arg("--remote-allow-origins=*")
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--disable-features=Translate")
        .arg("about:blank")
        .spawn()
        .map_err(|e| format!("크롬 실행 실패: {e}"))?;
    *state.chrome.lock().unwrap() = Some(child);
    let mut last = String::new();
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(300)).await;
        match do_connect(&app, port).await {
            Ok(()) => {
                engine::log(&app, "success", format!("크롬을 실행하고 연결했습니다 ({}, 프로필: {})", exe.display(), profile.display()));
                return Ok(browser_status_value(&state));
            }
            Err(e) => last = e,
        }
    }
    Err(format!("크롬을 실행했지만 디버깅 포트에 연결하지 못했습니다. 같은 프로필의 크롬이 이미 실행 중이라면 모두 닫고 다시 시도하세요. ({last})"))
}

#[tauri::command]
async fn browser_disconnect(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    if let Some(b) = state.browser.lock().unwrap().take() {
        b.close();
    }
    emit_browser_state(&app);
    Ok(())
}

#[tauri::command]
fn browser_status(state: State<'_, AppState>) -> Value {
    browser_status_value(&state)
}

#[tauri::command]
async fn browser_activate(app: AppHandle, state: State<'_, AppState>, target_id: String) -> Result<(), String> {
    let b = state.browser()?;
    let session = b.session_of_target(&target_id).ok_or("탭을 찾을 수 없습니다")?;
    b.set_active_session(&session);
    b.activate(&target_id).await?;
    emit_browser_state(&app);
    Ok(())
}

// ---------- settings / ollama ----------

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn save_settings(state: State<'_, AppState>, settings: Settings) -> Result<(), String> {
    std::fs::create_dir_all(&state.data_dir).map_err(|e| e.to_string())?;
    std::fs::write(state.settings_path(), serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    *state.settings.lock().unwrap() = settings;
    Ok(())
}

#[tauri::command]
async fn ollama_models(state: State<'_, AppState>) -> Result<Value, String> {
    let url = state.settings.lock().unwrap().ollama_url.clone();
    ollama::Ollama::new(&url).models().await
}

// ---------- scenarios ----------

#[tauri::command]
fn scenario_list(state: State<'_, AppState>) -> Vec<Value> {
    list_scenarios(&state.scenarios_dir())
        .into_iter()
        .map(|s| json!({"id": s.id, "name": s.name, "description": s.description, "updatedAt": s.updated_at, "stepCount": s.steps.len()}))
        .collect()
}

#[tauri::command]
fn scenario_load(state: State<'_, AppState>, id: String) -> Result<Scenario, String> {
    let mut s = load_scenario(&state.scenarios_dir(), &id)?;
    s.id = id;
    Ok(s)
}

#[tauri::command]
fn scenario_save(state: State<'_, AppState>, mut scenario: Scenario) -> Result<Scenario, String> {
    save_scenario(&state.scenarios_dir(), &mut scenario)?;
    Ok(scenario)
}

#[tauri::command]
fn scenario_delete(state: State<'_, AppState>, id: String) -> Result<(), String> {
    std::fs::remove_file(scenario_path(&state.scenarios_dir(), &id)?).map_err(|e| e.to_string())
}

#[tauri::command]
fn open_scenarios_dir(state: State<'_, AppState>) -> Result<String, String> {
    let dir = state.scenarios_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::process::Command::new("explorer").arg(&dir).spawn().map_err(|e| e.to_string())?;
    Ok(dir.display().to_string())
}

// ---------- recorder ----------

#[tauri::command]
async fn record_start(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    if state.running.load(Ordering::SeqCst) {
        return Err("실행 중에는 녹화할 수 없습니다".into());
    }
    let b = state.browser()?;
    let url = b.url().await;
    recorder::start(&app, &url);
    engine::log(&app, "info", "⏺ 녹화를 시작했습니다. 크롬 창에서 조작하세요");
    Ok(())
}

#[tauri::command]
fn record_stop(app: AppHandle, state: State<'_, AppState>) -> Vec<Step> {
    let steps = {
        let mut r = state.recorder.lock().unwrap();
        r.active = false;
        std::mem::take(&mut r.steps)
    };
    let _ = app.emit("record://state", json!({"recording": false}));
    engine::log(&app, "info", format!("⏹ 녹화 종료: {}개 스텝", steps.len()));
    steps
}

// ---------- execution ----------

fn begin_run(app: &AppHandle, state: &AppState, kind: &str) -> Result<Arc<Browser>, String> {
    let b = state.browser()?;
    if state.recorder.lock().unwrap().active {
        return Err("녹화 중에는 실행할 수 없습니다".into());
    }
    if state.running.swap(true, Ordering::SeqCst) {
        return Err("이미 실행 중입니다".into());
    }
    let _ = app.emit("run://state", json!({"running": true, "kind": kind}));
    Ok(b)
}

fn end_run(app: &AppHandle, kind: &str, result: &Result<String, String>) {
    let state = app.state::<AppState>();
    state.running.store(false, Ordering::SeqCst);
    for (_, tx) in state.confirms.lock().unwrap().drain() {
        let _ = tx.send(false);
    }
    for (_, tx) in state.cred_prompts.lock().unwrap().drain() {
        let _ = tx.send(None);
    }
    match result {
        Ok(m) => engine::log(app, "success", format!("✔ 실행 완료 {m}")),
        Err(e) => engine::log(app, "error", format!("✖ 실행 중단: {e}")),
    }
    let _ = app.emit(
        "run://state",
        json!({"running": false, "kind": kind, "ok": result.is_ok(), "message": match result { Ok(m) => m.clone(), Err(e) => e.clone() }}),
    );
}

#[tauri::command]
async fn run_scenario(app: AppHandle, state: State<'_, AppState>, mut scenario: Scenario) -> Result<(), String> {
    let b = begin_run(&app, &state, "scenario")?;
    let dir = state.scenarios_dir();
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut ctx = engine::Ctx::new(&app2, b, scenario.variables.clone());
        engine::log(&app2, "info", format!("▶ 시나리오 실행: {}", scenario.name));
        let started = std::time::Instant::now();
        let r = engine::run_steps(&mut ctx, &mut scenario.steps).await;
        if ctx.patched {
            match save_scenario(&dir, &mut scenario) {
                Ok(()) => {
                    engine::log(&app2, "success", "시나리오 파일을 자동 패치했습니다 (self-heal / compile-to-macro)");
                    let _ = app2.emit("scenario://patched", &scenario);
                }
                Err(e) => engine::log(&app2, "error", format!("시나리오 패치 저장 실패: {e}")),
            }
        }
        let _ = app2.emit("run://vars", Value::Object(ctx.vars.clone()));
        end_run(&app2, "scenario", &r.map(|_| format!("({:.1}초)", started.elapsed().as_secs_f32())));
    });
    Ok(())
}

/// Tier 3 standalone: run a natural-language goal and return the trace as macro steps.
#[tauri::command]
async fn agent_run(app: AppHandle, state: State<'_, AppState>, goal: String) -> Result<(), String> {
    let b = begin_run(&app, &state, "agent")?;
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut ctx = engine::Ctx::new(&app2, b, serde_json::Map::new());
        let start_url = ctx.browser.url().await;
        let max = ctx.settings.max_agent_steps;
        let r = agent::run_goal(&mut ctx, &goal, max).await;
        let result = match r {
            Ok(res) => {
                let mut steps = res.trace;
                let starts_with_nav = steps.first().and_then(|s| s.action.as_ref()).map(|a| a.kind == "navigate").unwrap_or(false);
                if !starts_with_nav && start_url.starts_with("http") {
                    steps.insert(
                        0,
                        Step {
                            name: format!("이동: {start_url}"),
                            action: Some(Action { kind: "navigate".into(), url: start_url.clone(), ..Default::default() }),
                            ..Default::default()
                        },
                    );
                }
                let _ = app2.emit("agent://trace", json!({"goal": goal, "success": res.success, "message": res.message, "steps": steps}));
                if res.success { Ok(res.message) } else { Err(res.message) }
            }
            Err(e) => {
                let _ = app2.emit("agent://trace", json!({"goal": goal, "success": false, "message": e, "steps": []}));
                Err(e)
            }
        };
        end_run(&app2, "agent", &result);
    });
    Ok(())
}

#[tauri::command]
fn run_stop(state: State<'_, AppState>) {
    state.cancel.store(true, Ordering::SeqCst);
}

#[tauri::command]
fn respond_confirm(state: State<'_, AppState>, id: String, approve: bool) {
    if let Some(tx) = state.confirms.lock().unwrap().remove(&id) {
        let _ = tx.send(approve);
    }
}

// ---------- credential vault ----------

/// Passwords are write-only from the UI's point of view: the list never contains them.
#[tauri::command]
fn cred_list(state: State<'_, AppState>) -> Result<Vec<Value>, String> {
    Ok(vault::load(&state.data_dir)?
        .into_iter()
        .map(|c| json!({"name": c.name, "site": c.site, "username": c.username, "hasPassword": !c.password.is_empty(), "updatedAt": c.updated_at}))
        .collect())
}

#[tauri::command]
fn cred_save(state: State<'_, AppState>, original: Option<String>, name: String, site: String, username: String, password: Option<String>) -> Result<(), String> {
    let password = password.filter(|p| !p.is_empty());
    if original.is_none() && password.is_none() {
        return Err("비밀번호를 입력하세요".into());
    }
    let name = if name.trim().is_empty() { vault::host_of(&site) } else { name.trim().to_string() };
    vault::upsert(&state.data_dir, original.as_deref(), &name, &site, &username, password.as_deref())
}

#[tauri::command]
fn cred_delete(state: State<'_, AppState>, name: String) -> Result<(), String> {
    vault::delete(&state.data_dir, &name)
}

/// Answer to a "cred://prompt" login dialog. `cancel` (or missing password) aborts the login.
#[tauri::command]
fn respond_cred_prompt(state: State<'_, AppState>, id: String, username: String, password: String, save: bool, cancel: bool) {
    if let Some(tx) = state.cred_prompts.lock().unwrap().remove(&id) {
        let _ = tx.send(if cancel || password.is_empty() { None } else { Some(vault::CredAnswer { username, password, save }) });
    }
}

// ---------- Module C inspection tools ----------

#[tauri::command]
async fn distill_page(state: State<'_, AppState>) -> Result<Value, String> {
    state.browser()?.jm("J.distill({textBudget:5000})").await
}

#[tauri::command]
async fn som_screenshot(state: State<'_, AppState>) -> Result<Value, String> {
    let b = state.browser()?;
    let d = b.jm("J.distill({textBudget:5000})").await?;
    b.jm("J.somDraw()").await?;
    let shot = b.screenshot().await;
    let _ = b.jm("J.somClear()").await;
    Ok(json!({"image": shot?, "distilled": d}))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // JM_DATA_DIR: run against a separate data folder (settings, scenarios, vault, Chrome profile)
            let data_dir = match std::env::var("JM_DATA_DIR") {
                Ok(d) if !d.trim().is_empty() => PathBuf::from(d.trim()),
                _ => app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from(".")),
            };
            let _ = std::fs::create_dir_all(data_dir.join("scenarios"));
            let settings: Settings = std::fs::read_to_string(data_dir.join("settings.json"))
                .ok()
                .and_then(|s| serde_json::from_str(s.trim_start_matches('\u{feff}')).ok())
                .unwrap_or_default();
            app.manage(AppState {
                data_dir,
                settings: Mutex::new(settings),
                browser: Mutex::new(None),
                chrome: Mutex::new(None),
                recorder: Mutex::new(recorder::Recorder::default()),
                running: AtomicBool::new(false),
                cancel: Arc::new(AtomicBool::new(false)),
                confirms: Mutex::new(HashMap::new()),
                cred_prompts: Mutex::new(HashMap::new()),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            browser_launch,
            browser_disconnect,
            browser_status,
            browser_activate,
            get_settings,
            save_settings,
            ollama_models,
            scenario_list,
            scenario_load,
            scenario_save,
            scenario_delete,
            open_scenarios_dir,
            record_start,
            record_stop,
            run_scenario,
            agent_run,
            run_stop,
            respond_confirm,
            cred_list,
            cred_save,
            cred_delete,
            respond_cred_prompt,
            distill_page,
            som_screenshot
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
