//! Tier 3: goal-oriented autonomous agent (Planner -> Navigator -> Validator) with macro trace recording.

use crate::engine::{type_into_last, Ctx};
use crate::ollama::agent_schema;
use crate::scenario::*;
use serde_json::{json, Value};

pub struct AgentResult {
    pub success: bool,
    pub message: String,
    /// successful actions as deterministic (Tier 1) steps — "Compile to Macro"
    pub trace: Vec<Step>,
}

const SENSITIVE: &[&str] = &[
    "결제", "구매", "주문하기", "주문 하기", "송금", "이체", "삭제", "탈퇴", "해지", "환불", "pay", "purchase", "buy now", "place order",
    "checkout", "delete", "remove account", "transfer", "unsubscribe",
];

fn is_sensitive(text: &str) -> bool {
    let t = text.to_lowercase();
    SENSITIVE.iter().any(|k| t.contains(k))
}

fn fp_label(fp: &Value) -> String {
    let name = fp["axName"].as_str().unwrap_or("");
    let text = fp["innerText"].as_str().unwrap_or("");
    let s = if !name.is_empty() { name } else { text };
    s.chars().take(40).collect()
}

fn action_step(kind: &str, name: String, fp: Option<Value>) -> Step {
    Step {
        kind: T_ACTION.into(),
        name,
        action: Some(Action { kind: kind.into(), ..Default::default() }),
        fingerprint: fp,
        ..Default::default()
    }
}

fn domain_allowed(url: &str, allowed: &str) -> bool {
    let list: Vec<String> = allowed.split(',').map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect();
    if list.is_empty() {
        return true;
    }
    let host = url.split("://").nth(1).unwrap_or("").split(['/', '?', '#', ':']).next().unwrap_or("").to_lowercase();
    list.iter().any(|d| host == *d || host.ends_with(&format!(".{d}")))
}

const NAV_SYSTEM: &str = r#"You are a web navigation agent that controls a real Chrome browser to achieve the user's goal.
Each turn you receive the goal, the action history, and the current page as a list of interactive elements:
  [12] <button text="검색하기" />   <- the number in brackets is the element index
Lines starting with text:/heading: are non-interactive page text for context.

Decide the SINGLE next action. Rules:
- CLICK: set target_index to the element index.
- TYPE: set target_index to an input/textarea index and input_text to the text. Existing content is replaced. TYPE does not submit; use PRESS_KEY afterwards if needed.
- PRESS_KEY: set input_text to the key name (Enter, Tab, Escape, ArrowDown, ArrowUp, PageDown, Backspace ...). The key goes to the focused element.
- SCROLL: set scroll_direction DOWN or UP to reveal more of the page.
- WAIT: wait for the page to finish loading/updating.
- NAVIGATE: set input_text to a full URL. Only when the goal requires a different site/page and there is no link to click.
- EXTRACT: set input_text to the information you read from the page that the goal asks for.
- NONE: no action (use with COMPLETED / FAILED / NEED_USER_HELP).
- goal_status: IN_PROGRESS while working. COMPLETED only when the goal is ALREADY achieved on the current page (action must be NONE). FAILED if the goal is impossible. NEED_USER_HELP for login, CAPTCHA, 2FA, or payment confirmation.
- Read the action history first. An action marked "OK ... the page changed" has ALREADY taken effect: never repeat it. Verify its effect in the current page content and move on to the next step (or finish).
- Elements with the same text are told apart by their context="..." attribute; choose the one whose context matches the goal.
- Fields you do not need must still be present: target_index 0, input_text "", scroll_direction "DOWN".
- Only use indexes that exist in the current element list. Never repeat an action that already failed the same way; try a different approach.
- Login: never invent or type IDs/passwords. When the goal needs a login and the page shows a login form (ID and/or password fields; some sites ask for the ID first and the password on the next screen), answer NEED_USER_HELP with action NONE; the system then signs in with the user's securely stored credentials.
- An element marked `disabled` cannot be clicked. If the goal cannot be achieved (required button disabled, item sold out, page missing), answer FAILED instead of idling with NONE.
- thought: ONE short sentence, always written in Korean (한국어), explaining the reason.
Respond with JSON only."#;

pub async fn run_goal(ctx: &mut Ctx, goal: &str, max_steps: u32) -> Result<AgentResult, String> {
    if goal.trim().is_empty() {
        return Err("목표(Goal)가 비어 있습니다".into());
    }
    if ctx.settings.text_model.trim().is_empty() && ctx.vision_model().is_none() {
        return Err("Ollama 모델이 선택되지 않았습니다".into());
    }
    let text_model = if ctx.settings.text_model.trim().is_empty() { ctx.settings.vision_model.clone() } else { ctx.settings.text_model.clone() };
    let max_steps = max_steps.clamp(1, 100);
    ctx.log("info", format!("🎯 에이전트 시작: {goal}"));

    // ---- Planner ----
    let mut plan = String::new();
    if ctx.settings.use_planner {
        let obs = ctx.observe(2500).await?;
        let schema = json!({"type": "object", "properties": {"plan": {"type": "array", "items": {"type": "string"}}}, "required": ["plan"]});
        let user = format!(
            "## Goal\n{goal}\n\n## Current page\nURL: {}\nTitle: {}\n\n## Elements\n{}\n\nWrite a short high-level plan (2-6 steps, Korean) to achieve the goal starting from this page.",
            obs["url"].as_str().unwrap_or(""),
            obs["title"].as_str().unwrap_or(""),
            obs["text"].as_str().unwrap_or("")
        );
        match ctx.ollama.chat_json(&text_model, "You are a planner for a web automation agent. Respond with JSON only.", &user, None, &schema).await {
            Ok(r) => {
                let items: Vec<String> = r["plan"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect()).unwrap_or_default();
                plan = items.iter().enumerate().map(|(i, s)| format!("{}. {s}", i + 1)).collect::<Vec<_>>().join("\n");
                ctx.log("info", format!("📋 계획:\n{plan}"));
            }
            Err(e) => ctx.log("warn", format!("플래너 실패(계획 없이 진행): {e}")),
        }
    }

    let mut history: Vec<String> = Vec::new();
    let mut trace: Vec<Step> = Vec::new();
    let mut last_sig = String::new();
    let mut repeat = 0u32;
    let mut rejections = 0u32;
    let mut login = LoginState::default();
    let mut prev_snapshot = String::new();
    let mut prev_text = String::new();
    let mut acted = false;
    // accidental double clicks: the element clicked in the previous step, whether that click changed the page,
    // and the element the model has already been warned about
    let mut last_click: Option<String> = None;
    let mut last_changed = false;
    let mut warned_click: Option<String> = None;

    for n in 1..=max_steps {
        ctx.check()?;
        // ---- Observe ----
        if n > 1 {
            ctx.browser.settle_dom(2500).await;
        }
        let obs = ctx.observe(3500).await?;
        let count = obs["count"].as_u64().unwrap_or(0);
        let url = obs["url"].as_str().unwrap_or("").to_string();
        // tell the model whether its previous action had a visible effect
        let snapshot = format!("{url}\n{}\n{}", obs["scrollY"], obs["text"].as_str().unwrap_or(""));
        if acted {
            last_changed = snapshot != prev_snapshot;
            if let Some(last) = history.last_mut() {
                if last_changed {
                    last.push_str(" — the page changed");
                    // show what the action produced, so the model sees its effect instead of repeating it
                    let added = new_lines(&prev_text, obs["text"].as_str().unwrap_or(""));
                    if !added.is_empty() {
                        last.push_str(&format!(". New on the page: {}", added.join(" | ")));
                    }
                } else {
                    last.push_str(" — the page did NOT change at all");
                }
            }
        }
        prev_snapshot = snapshot;
        prev_text = obs["text"].as_str().unwrap_or("").to_string();
        acted = false;
        let hist = if history.is_empty() {
            "(none yet)".to_string()
        } else {
            history.iter().rev().take(10).rev().cloned().collect::<Vec<_>>().join("\n")
        };
        let user = format!(
            "## Goal\n{goal}\n{}\n## Action history\n{hist}\n\n## Current page (step {n}/{max_steps})\nURL: {url}\nTitle: {}\nScroll: {}px of {}px (viewport {}px){}\n\n## Elements\n{}",
            if plan.is_empty() { String::new() } else { format!("\n## Plan\n{plan}\n") },
            obs["title"].as_str().unwrap_or(""),
            obs["scrollY"],
            obs["scrollHeight"],
            obs["viewportHeight"],
            if obs["truncated"].as_bool() == Some(true) { "\n(The element list is truncated to the area around the viewport; SCROLL to see more.)" } else { "" },
            obs["text"].as_str().unwrap_or("")
        );

        let has_login_form = ctx.browser.jm("J.loginForm()").await.map(|f| f["found"].as_bool() == Some(true)).unwrap_or(false);
        let login_ok = login.calls < MAX_LOGIN_CALLS;
        let user = if has_login_form && login_ok {
            format!("{user}\n\nNOTE: This page contains a login form (possibly one step of a multi-step login). Do NOT click or type into the ID/password fields yourself. If the goal requires signing in, answer NEED_USER_HELP with action NONE right now and the system will sign in with the user's stored credentials. If login is not needed for the goal, ignore the form.")
        } else {
            user
        };

        // ---- Think ----
        let schema = agent_schema(count);
        let use_vision = ctx.settings.vision_mode == "always" || (ctx.settings.vision_mode == "fallback" && count < 3);
        let resp = match (use_vision, ctx.vision_model()) {
            (true, Some(vm)) => {
                let shot = ctx.som_screenshot().await?;
                let vuser = format!("{user}\n\nThe attached screenshot shows the page; red boxes are labelled with the element [index].");
                ctx.ollama.chat_json(vm, NAV_SYSTEM, &vuser, Some(&shot), &schema).await?
            }
            _ => ctx.ollama.chat_json(&text_model, NAV_SYSTEM, &user, None, &schema).await?,
        };
        ctx.check()?;
        let thought = resp["thought"].as_str().unwrap_or("").to_string();
        let status = resp["goal_status"].as_str().unwrap_or("IN_PROGRESS").to_string();
        let act = &resp["action"];
        let atype = act["type"].as_str().unwrap_or("NONE").to_uppercase();
        let idx = act["target_index"].as_i64().unwrap_or(0);
        let input = act["input_text"].as_str().unwrap_or("").to_string();
        // small models often "wait for the system login" with IN_PROGRESS/NONE instead of asking for it
        let status = if has_login_form && login_ok && status == "IN_PROGRESS" && atype == "NONE" { "NEED_USER_HELP".to_string() } else { status };
        let _ = ctx.app.emit_agent(n, &thought, &status, &atype, idx, &input);
        ctx.log("info", format!("🤖 [{n}] {thought} → {status} / {atype}{}", if idx > 0 { format!(" [{idx}]") } else { String::new() }));

        match status.as_str() {
            "COMPLETED" => {
                // ---- Validator ----
                if !ctx.settings.use_validator {
                    return Ok(AgentResult { success: true, message: thought, trace });
                }
                let vschema = json!({"type": "object", "properties": {"reason": {"type": "string"}, "achieved": {"type": "boolean"}}, "required": ["reason", "achieved"]});
                let vuser = format!(
                    "## Goal\n{goal}\n\n## Actions performed\n{hist}\n\n## Agent claim\n{thought}\n\n## Current page\nURL: {url}\nTitle: {}\n\n## Page content\n{}\n\nIs the goal achieved, judging from the performed actions and the current page state? `reason` in Korean (one sentence).",
                    obs["title"].as_str().unwrap_or(""),
                    obs["text"].as_str().unwrap_or("")
                );
                let v = ctx
                    .ollama
                    .chat_json(&text_model, "You are a strict validator for a web automation agent. Decide whether the goal has really been achieved. Respond with JSON only.", &vuser, None, &vschema)
                    .await?;
                let reason = v["reason"].as_str().unwrap_or("").to_string();
                if v["achieved"].as_bool() == Some(true) {
                    ctx.log("success", format!("✅ 검증 통과: {reason}"));
                    return Ok(AgentResult { success: true, message: reason, trace });
                }
                rejections += 1;
                ctx.log("warn", format!("검증 실패({rejections}): {reason}"));
                if rejections >= 3 {
                    return Ok(AgentResult { success: false, message: format!("검증기를 통과하지 못했습니다: {reason}"), trace });
                }
                history.push(format!("{n}. VALIDATOR REJECTED completion: {reason}. Keep working."));
                continue;
            }
            "FAILED" => return Ok(AgentResult { success: false, message: format!("에이전트가 목표 달성 실패를 보고했습니다: {thought}"), trace }),
            "NEED_USER_HELP" => {
                // a login form is handled by the credential vault (stored login, or ask the user once)
                if login_ok {
                    login.calls += 1;
                    match try_login(ctx, &mut trace, &mut login).await {
                        Ok(Some(msg)) => {
                            acted = true;
                            history.push(format!("{n}. LOGIN → OK: {msg}. Do not ask for login again unless the page still shows the login form."));
                            continue;
                        }
                        Ok(None) => {}
                        Err(e) => ctx.log("warn", format!("자동 로그인 실패: {e}")),
                    }
                    ctx.check()?;
                }
                let ok = ctx
                    .confirm("에이전트가 도움을 요청했습니다", &format!("{thought}\n\n브라우저에서 직접 조치(로그인, CAPTCHA 등)한 뒤 [계속]을 누르세요."))
                    .await;
                if !ok {
                    return Ok(AgentResult { success: false, message: "사용자 도움 요청이 거부되었습니다".into(), trace });
                }
                history.push(format!("{n}. The user manually helped. Re-check the page and continue."));
                continue;
            }
            _ => {}
        }

        // ---- loop guard ----
        let sig = format!("{atype}|{idx}|{input}|{url}|{}", obs["scrollY"]);
        if sig == last_sig {
            repeat += 1;
            if repeat >= 3 {
                return Ok(AgentResult { success: false, message: "같은 행동을 반복하여 중단했습니다 (루프 감지)".into(), trace });
            }
        } else {
            repeat = 0;
            last_sig = sig;
        }

        // ---- Act ----
        let mut clicked_css: Option<String> = None;
        let outcome: Result<String, String> = async {
            match atype.as_str() {
                "CLICK" | "TYPE" => {
                    if idx < 1 || idx as u64 > count {
                        return Err(format!("index {idx} does not exist (valid: 1..{count})"));
                    }
                    let p = ctx.browser.jm(&format!("J.pick({idx})")).await?;
                    if p["status"].as_str() == Some("disabled") {
                        return Err(format!("element [{idx}] is DISABLED and cannot be used. If the goal depends on it, the goal is impossible: answer FAILED"));
                    }
                    if p["status"].as_str() != Some("ok") {
                        return Err(format!("element [{idx}] is not visible/usable"));
                    }
                    let fp = p["fingerprint"].clone();
                    let label = fp_label(&fp);
                    let (x, y) = (p["x"].as_f64().unwrap_or(0.0), p["y"].as_f64().unwrap_or(0.0));
                    if atype == "CLICK" {
                        if ctx.settings.confirm_sensitive && is_sensitive(&format!("{label} {}", fp["innerText"].as_str().unwrap_or(""))) {
                            let ok = ctx.confirm("민감한 동작 확인", &format!("에이전트가 「{label}」을(를) 클릭하려 합니다.\n이유: {thought}\n\n허용하시겠습니까?")).await;
                            if !ok {
                                return Err("the user denied this click; do not try it again".into());
                            }
                        }
                        // The same element again right after a click that already took effect is usually a mistake
                        // (item added twice, form submitted twice). Refuse once; a second identical request goes through.
                        let css = fp["primaryCss"].as_str().unwrap_or("").to_string();
                        if !css.is_empty() && last_changed && last_click.as_deref() == Some(css.as_str()) && warned_click.as_deref() != Some(css.as_str()) {
                            warned_click = Some(css);
                            return Err(format!(
                                "NOT clicked: you already clicked this same element \"{label}\" in the previous step and it took effect (the page changed). Clicking it again would repeat the action (e.g. add the item twice). Check the current page: if the goal is achieved answer COMPLETED. Only if the goal really needs another click on it, request the same click again"
                            ));
                        }
                        ctx.browser.click_at(x, y).await?;
                        ctx.browser.wait_settle(8000).await;
                        clicked_css = Some(css);
                        trace.push(action_step("click", format!("클릭: {label}"), Some(fp)));
                        Ok(format!("clicked [{idx}] \"{label}\""))
                    } else {
                        if p["password"].as_bool() == Some(true) {
                            return Err("this is a password field and you do not know the password. Answer NEED_USER_HELP: the system will then log in with the user's stored credentials".into());
                        }
                        if p["editable"].as_bool() != Some(true) && p["tag"].as_str() != Some("select") {
                            return Err(format!("element [{idx}] is not a text input"));
                        }
                        if p["tag"].as_str() == Some("select") {
                            let ok = ctx.browser.jm(&format!("J.selectValue({})", json!(input))).await?;
                            if ok.as_bool() != Some(true) {
                                return Err(format!("option \"{input}\" not found in select [{idx}]"));
                            }
                            let mut s = action_step("select", format!("선택: {label} = {input}"), Some(fp));
                            s.action.as_mut().unwrap().value = input.clone();
                            trace.push(s);
                        } else {
                            type_into_last(&ctx.browser, x, y, &input).await?;
                            let mut s = action_step("type", format!("입력: {label}"), Some(fp));
                            s.action.as_mut().unwrap().value = input.clone();
                            // re-typing into the same field replaces the earlier value: keep only the last one
                            let same = trace.last().map(|l| {
                                l.action.as_ref().map(|a| a.kind == "type").unwrap_or(false)
                                    && l.fingerprint.as_ref().map(|f| f["primaryCss"] == s.fingerprint.as_ref().unwrap()["primaryCss"]).unwrap_or(false)
                            });
                            if same == Some(true) {
                                trace.pop();
                            }
                            trace.push(s);
                        }
                        Ok(format!("typed \"{input}\" into [{idx}] \"{label}\""))
                    }
                }
                "PRESS_KEY" => {
                    let key = if input.trim().is_empty() { "Enter".to_string() } else { input.trim().to_string() };
                    ctx.browser.press_key(&key, 0).await?;
                    ctx.browser.wait_settle(8000).await;
                    let mut s = action_step("press_key", format!("키 입력: {key}"), None);
                    s.action.as_mut().unwrap().key = key.clone();
                    trace.push(s);
                    Ok(format!("pressed {key}"))
                }
                "SCROLL" => {
                    let up = act["scroll_direction"].as_str().unwrap_or("DOWN").eq_ignore_ascii_case("UP");
                    let before = obs["scrollY"].as_f64().unwrap_or(0.0);
                    let y = ctx
                        .browser
                        .eval(&format!("(window.scrollBy({{top:{}window.innerHeight*0.8,behavior:'instant'}}),Math.round(window.scrollY))", if up { "-" } else { "" }))
                        .await?
                        .as_f64()
                        .unwrap_or(0.0);
                    ctx.sleep(400).await?;
                    if (y - before).abs() < 2.0 {
                        return Err("the page cannot scroll further in that direction".into());
                    }
                    let mut s = action_step("scroll", format!("스크롤: {y}px"), None);
                    s.action.as_mut().unwrap().scroll_y = y;
                    trace.push(s);
                    Ok(format!("scrolled {} to {y}px", if up { "up" } else { "down" }))
                }
                "WAIT" => {
                    ctx.sleep(1500).await?;
                    let mut s = action_step("wait", "대기 1.5초".into(), None);
                    s.action.as_mut().unwrap().ms = 1500;
                    trace.push(s);
                    Ok("waited 1.5s".into())
                }
                "NAVIGATE" => {
                    let target = input.trim().to_string();
                    let target = if target.contains("://") { target } else { format!("https://{target}") };
                    if !(target.starts_with("http://") || target.starts_with("https://")) {
                        return Err("only http/https URLs are allowed".into());
                    }
                    if !domain_allowed(&target, &ctx.settings.allowed_domains) {
                        return Err(format!("navigation to this domain is blocked by the allow-list ({})", ctx.settings.allowed_domains));
                    }
                    ctx.browser.navigate(&target).await?;
                    let mut s = action_step("navigate", format!("이동: {target}"), None);
                    s.action.as_mut().unwrap().url = target.clone();
                    trace.push(s);
                    Ok(format!("navigated to {target}"))
                }
                "EXTRACT" => {
                    ctx.vars.insert("agent_extract".into(), json!(input));
                    let _ = ctx.app.emit_vars(&ctx.vars);
                    ctx.log("success", format!("에이전트 추출: {input}"));
                    Ok(format!("extracted: {input}"))
                }
                _ => {
                    ctx.sleep(500).await?;
                    Ok("no action".into())
                }
            }
        }
        .await;
        ctx.check()?;
        match outcome {
            Ok(m) => {
                acted = !matches!(atype.as_str(), "EXTRACT" | "NONE" | "WAIT");
                if acted {
                    if clicked_css.is_none() {
                        warned_click = None;
                    }
                    last_click = clicked_css;
                }
                history.push(format!("{n}. {atype} → OK: {m}"));
            }
            Err(e) => {
                ctx.log("warn", format!("행동 실패: {e}"));
                history.push(format!("{n}. {atype} [{idx}] → ERROR: {e}"));
            }
        }
    }
    Ok(AgentResult { success: false, message: format!("최대 스텝({max_steps})을 초과했습니다"), trace })
}

/// Page lines that appeared after an action (element indexes ignored), shortened for the action history.
fn new_lines(before: &str, after: &str) -> Vec<String> {
    fn strip(l: &str) -> &str {
        let l = l.trim();
        match (l.starts_with('['), l.find("] ")) {
            (true, Some(i)) => &l[i + 2..],
            _ => l,
        }
    }
    let old: std::collections::HashSet<&str> = before.lines().map(strip).collect();
    after
        .lines()
        .map(strip)
        .filter(|l| !l.is_empty() && !old.contains(l))
        .take(4)
        .map(|l| l.chars().take(140).collect::<String>())
        .collect()
}

/// How often the agent may hand a login screen to the credential vault in one run
/// (a two-step login can take two calls, plus one corrected retry for each step).
const MAX_LOGIN_CALLS: u32 = 4;

#[derive(Default)]
struct LoginState {
    calls: u32,
    /// how many times the login ID / the password has been submitted in this run
    id_sent: u32,
    pw_sent: u32,
}

/// Polls the page until a password field shows up after the ID step. Returns the last seen login stage
/// ("pass" / "full" when the password can be entered, otherwise "id" or "none").
async fn wait_password_step(ctx: &Ctx, max_ms: u64) -> Result<String, String> {
    let start = std::time::Instant::now();
    let mut stage = String::from("none");
    loop {
        ctx.sleep(300).await?;
        if let Ok(f) = ctx.browser.jm("J.loginForm()").await {
            stage = f["stage"].as_str().unwrap_or("none").to_string();
            if stage == "pass" || stage == "full" {
                return Ok(stage);
            }
        }
        if start.elapsed().as_millis() as u64 > max_ms {
            return Ok(stage);
        }
    }
}

/// If the active page shows a login form, sign in with the stored credential for this site, or ask the
/// user for ID/password (optionally saving them to the encrypted vault). Handles one-page forms and
/// two-step logins (ID screen -> "next" -> password screen). The password is typed straight into the
/// page; it never reaches the LLM, the log, or the recorded trace (which stores {{cred...}} references).
/// Returns Ok(None) when there is no login form or the user declined.
async fn try_login(ctx: &mut Ctx, trace: &mut Vec<Step>, st: &mut LoginState) -> Result<Option<String>, String> {
    let mut form = ctx.browser.jm("J.loginForm()").await?;
    if form["found"].as_bool() != Some(true) {
        return Ok(None);
    }
    let mut stage = form["stage"].as_str().unwrap_or("full").to_string();
    let host = crate::vault::host_of(form["host"].as_str().unwrap_or(""));
    if host.is_empty() {
        return Ok(None);
    }
    // the same screen again after we already submitted it: the ID / password was rejected
    let retry = if stage == "id" { st.id_sent > 0 } else { st.pw_sent > 0 };
    if retry && (st.id_sent >= 2 || st.pw_sent >= 2) {
        return Ok(None);
    }
    let dir = { use tauri::Manager; ctx.app.state::<crate::AppState>().data_dir.clone() };
    let stored = crate::vault::find_for_host(&dir, &host)?;
    let name = stored.as_ref().map(|c| c.name.clone()).unwrap_or_else(|| host.clone());
    // an answer given earlier in this run (possibly not saved) wins over the vault
    let session = ctx.session_creds.lock().unwrap().get(&name).cloned();
    let cred = match (session.or(stored), retry) {
        (Some(c), false) => {
            ctx.log("info", format!("🔐 로그인 정보 '{}'(으)로 로그인합니다 (아이디: {})", c.name, c.username));
            c
        }
        (known, _) => {
            let user = match &known {
                Some(c) => c.username.clone(),
                None => form["userValue"].as_str().unwrap_or("").to_string(),
            };
            let msg = if !retry {
                format!("{host} 에 로그인이 필요합니다. 아이디와 비밀번호를 입력해 주세요.")
            } else if stage == "id" {
                format!("{host} 에서 아이디가 받아들여지지 않은 것 같습니다. 아이디와 비밀번호를 다시 확인해 주세요.")
            } else {
                format!("{host} 로그인에 실패한 것 같습니다. 아이디와 비밀번호를 다시 확인해 주세요.")
            };
            match ctx.prompt_credential(&name, &host, &user, &msg).await? {
                Some(c) => c,
                None => return Ok(None),
            }
        }
    };
    // whatever the agent already tried on the login fields (invented IDs, focus clicks) must not end up in the macro
    let mut login_css: Vec<String> = Vec::new();
    for which in ["user", "pass"] {
        if let Ok(p) = ctx.browser.jm(&format!("J.loginField('{which}')")).await {
            if let Some(c) = p["fingerprint"]["primaryCss"].as_str() {
                login_css.push(c.to_string());
            }
        }
    }
    // ...and neither must an earlier, rejected attempt at this same screen ("next" clicks included)
    if retry {
        for which in ["user", "pass"] {
            if let Ok(p) = ctx.browser.jm(&format!("J.loginSubmit('{which}')")).await {
                if let Some(c) = p["fingerprint"]["primaryCss"].as_str() {
                    login_css.push(c.to_string());
                }
            }
        }
    }
    while trace.last().map(|s| s.fingerprint.as_ref().and_then(|f| f["primaryCss"].as_str()).map(|c| login_css.iter().any(|l| l == c)).unwrap_or(false)).unwrap_or(false) {
        trace.pop();
    }
    let user_ref = format!("{{{{cred.{}.username}}}}", cred.name);
    let enter_step = |name: &str| {
        let mut s = action_step("press_key", name.to_string(), None);
        s.action.as_mut().unwrap().key = "Enter".into();
        s
    };

    // ---- step 1 of a two-step login: only the ID field is on screen ----
    if stage == "id" {
        let p = ctx.browser.jm("J.loginField('user')").await?;
        if p["status"].as_str() != Some("ok") {
            return Err("아이디 입력란을 찾을 수 없습니다".into());
        }
        type_into_last(&ctx.browser, p["x"].as_f64().unwrap_or(0.0), p["y"].as_f64().unwrap_or(0.0), &cred.username).await?;
        let mut s = action_step("type", "입력: 로그인 아이디".into(), Some(p["fingerprint"].clone()));
        s.action.as_mut().unwrap().value = user_ref.clone();
        trace.push(s);
        st.id_sent += 1;
        ctx.browser.press_key("Enter", 0).await?;
        ctx.browser.wait_settle(8000).await;
        let mut submit = enter_step("키 입력: Enter (다음 단계)");
        let mut next = wait_password_step(ctx, 3000).await?;
        if next == "id" {
            // Enter did not move on: press the "next" button of the form instead
            if let Ok(b) = ctx.browser.jm("J.loginSubmit('user')").await {
                if b["status"].as_str() == Some("ok") {
                    let label = fp_label(&b["fingerprint"]);
                    ctx.browser.click_at(b["x"].as_f64().unwrap_or(0.0), b["y"].as_f64().unwrap_or(0.0)).await?;
                    ctx.browser.wait_settle(8000).await;
                    submit = action_step("click", format!("클릭: {label} (다음 단계)"), Some(b["fingerprint"].clone()));
                    next = wait_password_step(ctx, 8000).await?;
                }
            }
        }
        trace.push(submit);
        if next != "pass" && next != "full" {
            ctx.log("info", "아이디를 입력했지만 비밀번호 입력란이 아직 나타나지 않았습니다");
            return Ok(Some(format!(
                "the system entered the user's login ID \"{}\" and submitted the first login step, but no password field has appeared yet. Look at the page: if it now asks for a password, answer NEED_USER_HELP again; if it shows an error, CAPTCHA or extra verification, answer NEED_USER_HELP",
                cred.username
            )));
        }
        ctx.log("info", "2단계 로그인: 아이디 입력 완료, 비밀번호 단계로 진행합니다");
        // let the password screen finish its transition before clicking into it
        ctx.browser.settle_dom(1500).await;
        form = ctx.browser.jm("J.loginForm()").await?;
        stage = form["stage"].as_str().unwrap_or("pass").to_string();
    }

    // ---- password screen (one-page form, or step 2) ----
    if stage == "full" && form["userValue"].as_str() != Some(cred.username.as_str()) {
        let p = ctx.browser.jm("J.loginField('user')").await?;
        if p["status"].as_str() == Some("ok") {
            type_into_last(&ctx.browser, p["x"].as_f64().unwrap_or(0.0), p["y"].as_f64().unwrap_or(0.0), &cred.username).await?;
            let mut s = action_step("type", "입력: 로그인 아이디".into(), Some(p["fingerprint"].clone()));
            s.action.as_mut().unwrap().value = user_ref.clone();
            trace.push(s);
        }
    }
    let p = ctx.browser.jm("J.loginField('pass')").await?;
    if p["status"].as_str() != Some("ok") {
        return Err("비밀번호 입력란을 찾을 수 없습니다".into());
    }
    type_into_last(&ctx.browser, p["x"].as_f64().unwrap_or(0.0), p["y"].as_f64().unwrap_or(0.0), &cred.password).await?;
    let mut s = action_step("type", "입력: 로그인 비밀번호".into(), Some(p["fingerprint"].clone()));
    s.action.as_mut().unwrap().value = format!("{{{{cred.{}.password}}}}", cred.name);
    trace.push(s);
    st.pw_sent += 1;
    ctx.browser.press_key("Enter", 0).await?;
    ctx.browser.wait_settle(10000).await;
    trace.push(enter_step("키 입력: Enter (로그인)"));
    Ok(Some(format!("the system filled the login form with the user's stored credentials (ID \"{}\") and submitted it", cred.username)))
}

/// Small helpers so the agent can emit UI events without importing tauri traits everywhere.
pub trait AgentEmit {
    fn emit_agent(&self, n: u32, thought: &str, status: &str, action: &str, idx: i64, input: &str) -> Result<(), String>;
    fn emit_vars(&self, vars: &serde_json::Map<String, Value>) -> Result<(), String>;
}

impl AgentEmit for tauri::AppHandle {
    fn emit_agent(&self, n: u32, thought: &str, status: &str, action: &str, idx: i64, input: &str) -> Result<(), String> {
        use tauri::Emitter;
        self.emit("agent://step", json!({"n": n, "thought": thought, "status": status, "action": action, "index": idx, "input": input}))
            .map_err(|e| e.to_string())
    }
    fn emit_vars(&self, vars: &serde_json::Map<String, Value>) -> Result<(), String> {
        use tauri::Emitter;
        self.emit("run://vars", Value::Object(vars.clone())).map_err(|e| e.to_string())
    }
}
