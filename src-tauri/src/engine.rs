//! Module D: 3-tier execution engine (deterministic -> Ollama self-healing -> goal agent).

use crate::agent;
use crate::browser::Browser;
use crate::ollama::Ollama;
use crate::scenario::*;
use crate::vault::{self, CredAnswer, Credential};
use crate::AppState;
use serde_json::{json, Map, Value};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

pub struct Ctx {
    pub app: AppHandle,
    pub browser: Arc<Browser>,
    pub ollama: Ollama,
    pub settings: Settings,
    pub vars: Map<String, Value>,
    pub cancel: Arc<AtomicBool>,
    /// scenario was modified (self-heal / compile-to-macro) and must be saved
    pub patched: bool,
    /// description of the previously executed step (context for healing)
    pub prev: String,
    /// credentials entered during this run (kept in memory even when the user chose not to save them)
    pub session_creds: std::sync::Mutex<std::collections::HashMap<String, Credential>>,
}

pub fn log(app: &AppHandle, level: &str, message: impl Into<String>) {
    let _ = app.emit(
        "log",
        json!({"level": level, "message": message.into(), "time": chrono::Local::now().format("%H:%M:%S").to_string()}),
    );
}

impl Ctx {
    pub fn new(app: &AppHandle, browser: Arc<Browser>, vars: Map<String, Value>) -> Ctx {
        let state = app.state::<AppState>();
        let settings = state.settings.lock().unwrap().clone();
        state.cancel.store(false, Ordering::SeqCst);
        Ctx {
            app: app.clone(),
            browser,
            ollama: Ollama::new(&settings.ollama_url),
            settings,
            vars,
            cancel: state.cancel.clone(),
            patched: false,
            prev: String::new(),
            session_creds: Default::default(),
        }
    }

    pub fn log(&self, level: &str, message: impl Into<String>) {
        log(&self.app, level, message);
    }

    pub fn step(&self, id: &str, status: &str, message: &str) {
        let _ = self.app.emit("run://step", json!({"stepId": id, "status": status, "message": message}));
    }

    pub fn check(&self) -> Result<(), String> {
        if self.cancel.load(Ordering::SeqCst) {
            Err("사용자가 실행을 중지했습니다".into())
        } else if !self.browser.is_alive() {
            Err("브라우저 연결이 끊어졌습니다".into())
        } else {
            Ok(())
        }
    }

    pub async fn sleep(&self, ms: u64) -> Result<(), String> {
        let end = Instant::now() + Duration::from_millis(ms);
        while Instant::now() < end {
            self.check()?;
            tokio::time::sleep(Duration::from_millis(100.min(ms.max(1)))).await;
        }
        Ok(())
    }

    pub fn sub(&self, s: &str) -> String {
        substitute(s, &self.vars)
    }

    /// Ask the user to approve something in the UI. Resolves to false when the run is cancelled.
    pub async fn confirm(&self, title: &str, message: &str) -> bool {
        let id = new_id();
        let (tx, mut rx) = tokio::sync::oneshot::channel::<bool>();
        self.app.state::<AppState>().confirms.lock().unwrap().insert(id.clone(), tx);
        let _ = self.app.emit("run://confirm", json!({"id": id, "title": title, "message": message}));
        loop {
            tokio::select! {
                r = &mut rx => return r.unwrap_or(false),
                _ = tokio::time::sleep(Duration::from_millis(200)) => {
                    if self.cancel.load(Ordering::SeqCst) {
                        self.app.state::<AppState>().confirms.lock().unwrap().remove(&id);
                        let _ = self.app.emit("run://confirm-close", json!({"id": id}));
                        return false;
                    }
                }
            }
        }
    }

    fn data_dir(&self) -> std::path::PathBuf {
        self.app.state::<AppState>().data_dir.clone()
    }

    /// Ask the user for a login ID / password in the app window. Returns None when cancelled.
    /// The answer is stored in the encrypted vault when the user ticks "save".
    pub async fn prompt_credential(&self, name: &str, site: &str, username: &str, message: &str) -> Result<Option<Credential>, String> {
        let id = new_id();
        let (tx, mut rx) = tokio::sync::oneshot::channel::<Option<CredAnswer>>();
        self.app.state::<AppState>().cred_prompts.lock().unwrap().insert(id.clone(), tx);
        let _ = self.app.emit("cred://prompt", json!({"id": id, "name": name, "site": site, "username": username, "message": message}));
        let answer = loop {
            tokio::select! {
                r = &mut rx => break r.unwrap_or(None),
                _ = tokio::time::sleep(Duration::from_millis(200)) => {
                    if self.cancel.load(Ordering::SeqCst) {
                        self.app.state::<AppState>().cred_prompts.lock().unwrap().remove(&id);
                        let _ = self.app.emit("cred://prompt-close", json!({"id": id}));
                        break None;
                    }
                }
            }
        };
        let Some(a) = answer else { return Ok(None) };
        let cred = Credential { name: name.to_string(), site: vault::host_of(site), username: a.username, password: a.password, updated_at: now() };
        if a.save {
            vault::upsert(&self.data_dir(), None, &cred.name, &cred.site, &cred.username, Some(&cred.password))?;
            let _ = self.app.emit("cred://changed", json!({}));
            self.log("success", format!("로그인 정보 '{}'을(를) 암호화하여 저장했습니다", cred.name));
        }
        self.session_creds.lock().unwrap().insert(cred.name.clone(), cred.clone());
        Ok(Some(cred))
    }

    /// Credential by reference name: this run's answers, then the vault, then ask the user.
    pub async fn credential(&self, name: &str) -> Result<Credential, String> {
        if let Some(c) = self.session_creds.lock().unwrap().get(name) {
            return Ok(c.clone());
        }
        if let Some(c) = vault::get(&self.data_dir(), name)? {
            return Ok(c);
        }
        if !vault::valid_name(name) {
            return Err(format!("잘못된 로그인 정보 이름: {name}"));
        }
        self.prompt_credential(name, name, "", &format!("시나리오가 로그인 정보 '{name}'을(를) 사용하지만 아직 저장되어 있지 않습니다."))
            .await?
            .ok_or_else(|| format!("로그인 정보 '{name}' 입력이 취소되었습니다"))
    }

    /// Like `sub`, but also resolves {{cred.<name>.username|password}}. Only for text that is typed
    /// into the page: never put the result into logs, LLM prompts or UI events.
    pub async fn sub_secret(&self, s: &str) -> Result<String, String> {
        let mut creds = std::collections::HashMap::new();
        for name in cred_refs(s) {
            let c = self.credential(&name).await?;
            creds.insert(name, c);
        }
        Ok(substitute_with(s, |token| match parse_cred_token(token) {
            Some((name, field)) => creds.get(&name).map(|c| if field == "username" { c.username.clone() } else { c.password.clone() }),
            None => {
                let whole = format!("{{{{{token}}}}}");
                let v = substitute(&whole, &self.vars);
                if v == whole { None } else { Some(v) }
            }
        }))
    }

    /// Module C: distilled interactive DOM of the active tab.
    pub async fn observe(&self, text_budget: u32) -> Result<Value, String> {
        let mut last = String::new();
        for _ in 0..4 {
            match self.browser.jm(&format!("J.distill({{textBudget:{text_budget}}})")).await {
                Ok(v) if v.is_object() => return Ok(v),
                Ok(_) => last = "페이지 분석 결과가 비어 있습니다".into(),
                Err(e) => last = e,
            }
            self.sleep(400).await?;
        }
        Err(format!("페이지 DOM을 분석할 수 없습니다: {last}"))
    }

    /// Set-of-Mark screenshot for the current distilled list.
    pub async fn som_screenshot(&self) -> Result<String, String> {
        self.browser.jm("J.somDraw()").await?;
        let shot = self.browser.screenshot().await;
        let _ = self.browser.jm("J.somClear()").await;
        shot
    }

    pub fn vision_model(&self) -> Option<&str> {
        if self.settings.vision_mode == "never" || self.settings.vision_model.trim().is_empty() {
            None
        } else {
            Some(self.settings.vision_model.as_str())
        }
    }
}

fn fp_str<'a>(fp: &'a Value, key: &str) -> &'a str {
    fp.get(key).and_then(|v| v.as_str()).unwrap_or("")
}

pub fn describe_step(step: &Step) -> String {
    if !step.name.is_empty() {
        return step.name.clone();
    }
    let short = |s: &str| s.chars().take(50).collect::<String>();
    match step.kind.as_str() {
        T_CONDITION => format!("LLM 조건: {}", short(&step.question)),
        T_EXTRACT => format!("LLM 추출: {}", short(&step.instruction)),
        T_GOAL => format!("AI 목표: {}", short(&step.goal)),
        T_LOOP => "반복".to_string(),
        _ => match &step.action {
            Some(a) => match a.kind.as_str() {
                "navigate" => format!("이동: {}", short(&a.url)),
                "wait" => format!("대기 {}ms", a.ms),
                "press_key" => format!("키 입력: {}", a.key),
                "scroll" => format!("스크롤: {}px", a.scroll_y),
                k => {
                    let label = match k {
                        "click" => "클릭",
                        "type" => "입력",
                        "select" => "선택",
                        other => other,
                    };
                    let fp = step.fingerprint.as_ref();
                    let name = fp.map(|f| fp_str(f, "axName")).filter(|s| !s.is_empty()).or(fp.map(|f| fp_str(f, "primaryCss"))).unwrap_or("");
                    format!("{label}: {}", short(name))
                }
            },
            None => step.kind.clone(),
        },
    }
}

// ---------- Tier 1 / Tier 2 element location ----------

struct Located {
    x: f64,
    y: f64,
    /// 1 = deterministic, 2 = healed by LLM
    tier: u8,
    /// fresh fingerprint to patch into the scenario after the action succeeds
    new_fp: Option<Value>,
    note: String,
}

async fn locate(ctx: &Ctx, step: &Step) -> Result<Located, String> {
    let fp = step.fingerprint.clone().ok_or("요소 정보(fingerprint)가 없는 스텝입니다")?;
    let timeout = if step.timeout_ms > 0 { step.timeout_ms } else { ctx.settings.default_timeout_ms };
    let start = Instant::now();
    let expr = format!("J.resolve({fp})");
    let mut status = "notfound".to_string();
    loop {
        ctx.check()?;
        if let Ok(r) = ctx.browser.jm(&expr).await {
            status = r["status"].as_str().unwrap_or("notfound").to_string();
            if status == "ok" {
                let method = r["method"].as_str().unwrap_or("css").to_string();
                let mut new_fp = None;
                let mut note = String::new();
                if method != "css" {
                    // primary selector is stale but a secondary identifier still matched: refresh it
                    new_fp = ctx.browser.jm("J.fingerprintLast()").await.ok().filter(|v| v.is_object());
                    note = format!("보조 식별자({method})로 탐색");
                }
                return Ok(Located { x: r["x"].as_f64().unwrap_or(0.0), y: r["y"].as_f64().unwrap_or(0.0), tier: 1, new_fp, note });
            }
        }
        if start.elapsed().as_millis() as u64 >= timeout {
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    if !step.llm_fallback {
        return Err(format!("요소를 찾지 못했습니다({status}). LLM 폴백이 꺼져 있습니다"));
    }
    if ctx.settings.text_model.trim().is_empty() && ctx.vision_model().is_none() {
        return Err(format!("요소를 찾지 못했습니다({status}). Ollama 모델이 선택되지 않아 자가 치유를 할 수 없습니다"));
    }
    ctx.step(&step.id, "healing", "Tier 2: Ollama 자가 치유 중…");
    ctx.log("warn", format!("Tier 1 실패({status}) → Tier 2 Ollama 자가 치유: {}", describe_step(step)));
    heal(ctx, step, &fp).await
}

async fn heal(ctx: &Ctx, step: &Step, fp: &Value) -> Result<Located, String> {
    let obs = ctx.observe(3000).await?;
    if obs["count"].as_u64().unwrap_or(0) == 0 && ctx.vision_model().is_none() {
        return Err("현재 페이지에 상호작용 가능한 요소가 없습니다".into());
    }
    let action = step.action.as_ref().map(|a| a.kind.as_str()).unwrap_or("click");
    let system = "You repair broken web-automation macros. A recorded element can no longer be found because the page UI changed \
(the element moved, or its id/class names changed). From the list of current interactive elements, pick the ONE element that serves the same purpose \
as the original. Match by meaning (label, role, surrounding context), not by exact string. If no element plausibly matches, return index -1. \
Respond with JSON only; write `reason` in Korean (one short sentence).";
    let user = format!(
        "## Original element (recorded earlier)\n- tag: {}\n- role: {}\n- accessible name: \"{}\"\n- inner text: \"{}\"\n- surrounding text: \"{}\"\n- attributes: {}\n- action to perform: {}\n- previous step: {}\n\n## Current page\nURL: {}\nTitle: {}\n\n## Current interactive elements\n{}\n\nReturn the [index] of the element that plays the same role as the original element.",
        fp_str(fp, "tag"),
        fp_str(fp, "axRole"),
        fp_str(fp, "axName"),
        fp_str(fp, "innerText"),
        fp_str(fp, "contextText"),
        fp.get("attrs").cloned().unwrap_or(json!({})),
        action,
        if ctx.prev.is_empty() { "(none)" } else { &ctx.prev },
        obs["url"].as_str().unwrap_or(""),
        obs["title"].as_str().unwrap_or(""),
        obs["text"].as_str().unwrap_or("")
    );
    let schema = json!({
        "type": "object",
        "properties": {"reason": {"type": "string"}, "index": crate::ollama::index_enum(-1, obs["count"].as_u64().unwrap_or(0))},
        "required": ["reason", "index"]
    });

    let mut index: i64 = -1;
    let mut reason = String::new();
    let vision_first = ctx.settings.vision_mode == "always" && ctx.vision_model().is_some();
    if !vision_first && !ctx.settings.text_model.trim().is_empty() {
        let r = ctx.ollama.chat_json(&ctx.settings.text_model, system, &user, None, &schema).await?;
        index = r["index"].as_i64().unwrap_or(-1);
        reason = r["reason"].as_str().unwrap_or("").to_string();
    }
    ctx.check()?;
    if index < 1 {
        if let Some(vm) = ctx.vision_model() {
            ctx.log("info", "DOM 기반 탐색 실패 → Set-of-Mark 스크린샷으로 비전 모델에 질의합니다");
            let shot = ctx.som_screenshot().await?;
            let vuser = format!("{user}\n\nThe attached screenshot shows the page with red boxes; each box is labelled with its [index].");
            let r = ctx.ollama.chat_json(vm, system, &vuser, Some(&shot), &schema).await?;
            index = r["index"].as_i64().unwrap_or(-1);
            reason = r["reason"].as_str().unwrap_or("").to_string();
        }
    }
    if index < 1 {
        return Err(format!("자가 치유 실패: 동일한 역할의 요소를 찾지 못했습니다. {reason}"));
    }
    let picked = ctx.browser.jm(&format!("J.pick({index})")).await?;
    if picked["status"].as_str() != Some("ok") {
        return Err(format!("자가 치유 실패: LLM이 고른 요소 [{index}]를 사용할 수 없습니다"));
    }
    Ok(Located {
        x: picked["x"].as_f64().unwrap_or(0.0),
        y: picked["y"].as_f64().unwrap_or(0.0),
        tier: 2,
        new_fp: Some(picked["fingerprint"].clone()),
        note: format!("Ollama 자가 치유 → [{index}] {reason}"),
    })
}

// ---------- deterministic action ----------

/// Types `text` into the element remembered as `J.last` (already located / clicked).
pub async fn type_into_last(browser: &Browser, x: f64, y: f64, text: &str) -> Result<(), String> {
    browser.click_at(x, y).await?;
    tokio::time::sleep(Duration::from_millis(80)).await;
    browser.jm("J.prepareType()").await?;
    if text.is_empty() {
        browser.press_key("Delete", 0).await
    } else {
        browser.insert_text(text).await
    }
}

async fn exec_action(ctx: &mut Ctx, step: &mut Step) -> Result<String, String> {
    let a = step.action.clone().ok_or("액션이 정의되지 않은 스텝입니다")?;
    match a.kind.as_str() {
        "navigate" => {
            let url = ctx.sub(&a.url);
            if url.trim().is_empty() {
                return Err("이동할 URL이 비어 있습니다".into());
            }
            let url = if url.contains("://") || url.starts_with("about:") { url } else { format!("https://{url}") };
            ctx.browser.navigate(&url).await?;
            Ok(String::new())
        }
        "wait" => {
            ctx.sleep(a.ms.max(1)).await?;
            Ok(String::new())
        }
        "press_key" => {
            ctx.browser.press_key(&a.key, a.modifiers).await?;
            ctx.browser.wait_settle(8000).await;
            Ok(String::new())
        }
        "scroll" => {
            ctx.browser.eval(&format!("window.scrollTo({{left:{},top:{},behavior:'instant'}})", a.scroll_x, a.scroll_y)).await?;
            ctx.sleep(300).await?;
            Ok(String::new())
        }
        "click" | "type" | "select" => {
            let loc = locate(ctx, step).await?;
            match a.kind.as_str() {
                "click" => {
                    ctx.browser.click_at(loc.x, loc.y).await?;
                    ctx.browser.wait_settle(8000).await;
                }
                "type" => {
                    let text = ctx.sub_secret(&a.value).await?;
                    type_into_last(&ctx.browser, loc.x, loc.y, &text).await?;
                }
                _ => {
                    let v = ctx.sub(&a.value);
                    let ok = ctx.browser.jm(&format!("J.selectValue({})", json!(v))).await?;
                    if ok.as_bool() != Some(true) {
                        return Err(format!("선택 항목 '{v}'을(를) 찾지 못했습니다"));
                    }
                }
            }
            if let Some(fp) = loc.new_fp {
                // self-heal: patch the scenario with the element that actually worked
                step.fingerprint = Some(fp);
                ctx.patched = true;
            }
            Ok(if loc.tier == 2 { format!("HEALED:{}", loc.note) } else { loc.note })
        }
        other => Err(format!("알 수 없는 액션: {other}")),
    }
}

async fn run_action_step(ctx: &mut Ctx, step: &mut Step) -> Result<(), String> {
    let attempts = step.retries + 1;
    let mut last = String::new();
    for n in 1..=attempts {
        ctx.check()?;
        match exec_action(ctx, step).await {
            Ok(note) => {
                if let Some(h) = note.strip_prefix("HEALED:") {
                    ctx.step(&step.id, "healed", h);
                    ctx.log("success", format!("자가 치유 성공, 셀렉터를 패치했습니다: {} ({h})", describe_step(step)));
                } else {
                    ctx.step(&step.id, "ok", &note);
                    if !note.is_empty() {
                        ctx.log("info", format!("{}: {note} → 셀렉터 갱신", describe_step(step)));
                    }
                }
                return Ok(());
            }
            Err(e) => {
                ctx.check()?;
                last = e;
                if n < attempts {
                    ctx.log("warn", format!("재시도 {n}/{}: {last}", step.retries));
                    ctx.sleep(700).await?;
                }
            }
        }
    }
    Err(last)
}

// ---------- LLM steps ----------

pub async fn llm_condition(ctx: &Ctx, question: &str, options: &[String]) -> Result<(String, String), String> {
    let obs = ctx.observe(8000).await?;
    let opts: Vec<String> = if options.is_empty() { vec!["true".into(), "false".into()] } else { options.to_vec() };
    let system = "You inspect the current state of a web page and answer a question about it. \
Base the answer strictly on the provided page content. Choose exactly one of the allowed answers. Respond with JSON only; write `reason` in Korean (one sentence).";
    let user = format!(
        "## Question\n{}\n\n## Allowed answers\n{}\n\n## Current page\nURL: {}\nTitle: {}\n\n## Page content (interactive elements are indexed, plain text prefixed with text:)\n{}",
        question,
        opts.join(" | "),
        obs["url"].as_str().unwrap_or(""),
        obs["title"].as_str().unwrap_or(""),
        obs["text"].as_str().unwrap_or("")
    );
    let schema = json!({
        "type": "object",
        "properties": {"reason": {"type": "string"}, "answer": {"type": "string", "enum": opts}},
        "required": ["reason", "answer"]
    });
    let use_vision = ctx.settings.vision_mode == "always" || (ctx.settings.vision_mode == "fallback" && obs["count"].as_u64().unwrap_or(0) < 3);
    let r = match (use_vision, ctx.vision_model()) {
        (true, Some(vm)) => {
            let shot = ctx.browser.screenshot().await?;
            ctx.ollama.chat_json(vm, system, &user, Some(&shot), &schema).await?
        }
        _ => ctx.ollama.chat_json(&ctx.settings.text_model, system, &user, None, &schema).await?,
    };
    let answer = r["answer"].as_str().unwrap_or("").trim().to_string();
    let answer = opts
        .iter()
        .find(|o| o.eq_ignore_ascii_case(&answer))
        .cloned()
        .ok_or_else(|| format!("LLM이 허용되지 않은 답을 반환했습니다: {answer}"))?;
    Ok((answer, r["reason"].as_str().unwrap_or("").to_string()))
}

/// Accepts a real JSON Schema, or a shorthand like {"title":"string","price":"number"} / [{"title":"string"}].
fn normalize_schema(schema: &Option<Value>) -> (Value, bool) {
    fn shorthand(obj: &Map<String, Value>) -> Value {
        let mut props = Map::new();
        for (k, v) in obj {
            let t = match v {
                Value::String(s) => {
                    let s = s.to_lowercase();
                    let ty = if ["number", "integer", "boolean", "string"].contains(&s.as_str()) { s } else { "string".into() };
                    json!({"type": ty})
                }
                Value::Object(o) if o.contains_key("type") => v.clone(),
                Value::Object(o) => shorthand(o),
                Value::Array(a) => match a.first() {
                    Some(Value::Object(o)) => json!({"type": "array", "items": shorthand(o)}),
                    _ => json!({"type": "array", "items": {"type": "string"}}),
                },
                _ => json!({"type": "string"}),
            };
            props.insert(k.clone(), t);
        }
        let req: Vec<&String> = obj.keys().collect();
        json!({"type": "object", "properties": props, "required": req})
    }
    match schema {
        Some(Value::Object(o)) if o.get("type").map(|t| t.is_string()).unwrap_or(false) && (o.contains_key("properties") || o.contains_key("items")) => {
            if o["type"] == "array" {
                (json!({"type": "object", "properties": {"items": Value::Object(o.clone())}, "required": ["items"]}), true)
            } else {
                (Value::Object(o.clone()), false)
            }
        }
        Some(Value::Object(o)) if !o.is_empty() => (shorthand(o), false),
        Some(Value::Array(a)) => match a.first() {
            Some(Value::Object(o)) => (
                json!({"type": "object", "properties": {"items": {"type": "array", "items": shorthand(o)}}, "required": ["items"]}),
                true,
            ),
            _ => (json!({"type": "object", "properties": {"items": {"type": "array", "items": {"type": "string"}}}, "required": ["items"]}), true),
        },
        _ => (json!("json"), false),
    }
}

async fn llm_extract(ctx: &Ctx, step: &Step) -> Result<Value, String> {
    let text = ctx.browser.jm("J.pageText(14000)").await?;
    let url = ctx.browser.url().await;
    let (schema, unwrap) = normalize_schema(&step.schema);
    let system = "You extract structured data from a web page. Use only information present in the page text; \
use empty strings / empty arrays for missing data. Never invent values. Respond with JSON only.";
    let user = format!(
        "## Extraction instruction\n{}\n\n## Output JSON schema\n{}\n\n## Page\nURL: {}\n\n## Page text\n{}",
        ctx.sub(&step.instruction),
        if schema.is_string() { "(free-form JSON object)".to_string() } else { schema.to_string() },
        url,
        text.as_str().unwrap_or("")
    );
    let r = ctx.ollama.chat_json(&ctx.settings.text_model, system, &user, None, &schema).await?;
    Ok(if unwrap { r.get("items").cloned().unwrap_or(r) } else { r })
}

// ---------- scenario runner ----------

type BoxFut<'a> = Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>>;

pub fn run_steps<'a>(ctx: &'a mut Ctx, steps: &'a mut Vec<Step>) -> BoxFut<'a> {
    Box::pin(async move {
        let mut i = 0;
        while i < steps.len() {
            ctx.check()?;
            if !steps[i].enabled {
                ctx.step(&steps[i].id, "skipped", "");
                i += 1;
                continue;
            }
            let id = steps[i].id.clone();
            let desc = describe_step(&steps[i]);
            ctx.step(&id, "running", "");
            let mut advance = 1usize;
            let result: Result<(), String> = match steps[i].kind.as_str() {
                T_ACTION => run_action_step(ctx, &mut steps[i]).await,
                T_CONDITION => {
                    let q = ctx.sub(&steps[i].question);
                    match llm_condition(ctx, &q, &steps[i].options).await {
                        Ok((answer, reason)) => {
                            ctx.log("info", format!("조건 판단 「{q}」 → {answer} ({reason})"));
                            ctx.step(&id, "ok", &format!("→ {answer}: {reason}"));
                            ctx.vars.insert("last_condition".into(), json!(answer));
                            match steps[i].branches.get_mut(&answer) {
                                Some(branch) if !branch.is_empty() => run_steps(ctx, branch).await,
                                _ => Ok(()),
                            }
                        }
                        Err(e) => Err(e),
                    }
                }
                T_EXTRACT => match llm_extract(ctx, &steps[i]).await {
                    Ok(v) => {
                        let var = if steps[i].output_var.trim().is_empty() { "extracted".to_string() } else { steps[i].output_var.trim().to_string() };
                        let preview: String = v.to_string().chars().take(400).collect();
                        ctx.log("success", format!("데이터 추출 → {{{{{var}}}}} = {preview}"));
                        ctx.step(&id, "ok", &format!("{var} = {}", preview.chars().take(120).collect::<String>()));
                        ctx.vars.insert(var, v);
                        let _ = ctx.app.emit("run://vars", Value::Object(ctx.vars.clone()));
                        Ok(())
                    }
                    Err(e) => Err(e),
                },
                T_GOAL => {
                    let goal = ctx.sub(&steps[i].goal);
                    let max = if steps[i].max_steps > 0 { steps[i].max_steps } else { ctx.settings.max_agent_steps };
                    match agent::run_goal(ctx, &goal, max).await {
                        Ok(res) if res.success => {
                            ctx.step(&id, "ok", &res.message);
                            if steps[i].compile_to_macro && !res.trace.is_empty() {
                                let n = res.trace.len();
                                steps.splice(i..=i, res.trace);
                                ctx.patched = true;
                                advance = n;
                                ctx.log("success", format!("AI 목표 스텝을 고속 매크로 스텝 {n}개로 변환했습니다 (Compile to Macro)"));
                            }
                            Ok(())
                        }
                        Ok(res) => Err(res.message),
                        Err(e) => Err(e),
                    }
                }
                T_LOOP => run_loop(ctx, &mut steps[i]).await.map(|n| ctx.step(&id, "ok", &format!("{n}회 반복"))),
                other => Err(format!("알 수 없는 스텝 타입: {other}")),
            };
            if let Err(e) = result {
                ctx.check()?;
                ctx.step(&id, "failed", &e);
                let cont = steps.get(i).map(|s| s.id == id && s.on_error == "continue").unwrap_or(false);
                if cont {
                    ctx.log("warn", format!("스텝 실패(계속 진행): {desc} — {e}"));
                } else {
                    return Err(format!("{desc}: {e}"));
                }
            }
            ctx.prev = desc;
            i += advance;
            if ctx.settings.step_delay_ms > 0 {
                ctx.sleep(ctx.settings.step_delay_ms).await?;
            }
        }
        Ok(())
    })
}

async fn run_loop(ctx: &mut Ctx, step: &mut Step) -> Result<u32, String> {
    let cfg = step.loop_cfg.clone().unwrap_or_default();
    let max = if cfg.max_iterations > 0 { cfg.max_iterations } else { 50 };
    let mut n = 0u32;
    match cfg.mode.as_str() {
        "list" => {
            let var = cfg.list_var.trim().trim_start_matches("{{").trim_end_matches("}}").trim().to_string();
            let list = match ctx.vars.get(&var) {
                Some(Value::Array(a)) => a.clone(),
                Some(Value::Object(o)) => o.values().find_map(|v| v.as_array().cloned()).ok_or(format!("변수 {var}에 목록이 없습니다"))?,
                Some(Value::String(s)) => s.split(|c| c == ',' || c == '\n').map(|x| json!(x.trim())).filter(|x| x != "").collect(),
                _ => return Err(format!("반복할 목록 변수 '{var}'가 없습니다")),
            };
            let item_var = if cfg.item_var.trim().is_empty() { "item".to_string() } else { cfg.item_var.trim().to_string() };
            for (idx, item) in list.into_iter().enumerate() {
                if n >= max {
                    break;
                }
                ctx.vars.insert(item_var.clone(), item);
                ctx.vars.insert("loop_index".into(), json!(idx));
                ctx.log("info", format!("반복 {}회차", idx + 1));
                run_steps(ctx, &mut step.children).await?;
                n += 1;
            }
        }
        "while" => {
            let opts = vec!["true".to_string(), "false".to_string()];
            while n < max {
                let q = ctx.sub(&cfg.condition);
                let (answer, reason) = llm_condition(ctx, &q, &opts).await?;
                ctx.log("info", format!("반복 조건 「{q}」 → {answer} ({reason})"));
                if answer != "true" {
                    break;
                }
                ctx.vars.insert("loop_index".into(), json!(n));
                run_steps(ctx, &mut step.children).await?;
                n += 1;
            }
        }
        _ => {
            let count = cfg.count.max(1).min(max);
            while n < count {
                ctx.vars.insert("loop_index".into(), json!(n));
                ctx.log("info", format!("반복 {}/{count}", n + 1));
                run_steps(ctx, &mut step.children).await?;
                n += 1;
            }
        }
    }
    Ok(n)
}
