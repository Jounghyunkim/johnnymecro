//! Module A: turns page events (sent through the CDP binding) into scenario steps.

use crate::browser::Browser;
use crate::scenario::*;
use crate::AppState;
use serde_json::{json, Value};
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Default)]
pub struct Recorder {
    pub active: bool,
    pub steps: Vec<Step>,
    last_event: Option<Instant>,
    /// primaryCss of typed fields that look like a login ID field (two-step logins)
    id_hints: Vec<String>,
}

fn label(fp: &Value) -> String {
    let name = fp["axName"].as_str().unwrap_or("");
    let text = fp["innerText"].as_str().unwrap_or("");
    let s = if !name.is_empty() { name } else if !text.is_empty() { text } else { fp["tag"].as_str().unwrap_or("요소") };
    s.chars().take(40).collect()
}

fn css(step: &Step) -> &str {
    step.fingerprint.as_ref().and_then(|f| f["primaryCss"].as_str()).unwrap_or("")
}

fn kind(step: &Step) -> &str {
    step.action.as_ref().map(|a| a.kind.as_str()).unwrap_or("")
}

fn nav_step(url: &str) -> Step {
    Step {
        name: format!("이동: {}", url.chars().take(80).collect::<String>()),
        action: Some(Action { kind: "navigate".into(), url: url.to_string(), ..Default::default() }),
        ..Default::default()
    }
}

fn recordable_url(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://") || url.starts_with("file://")
}

fn publish(app: &AppHandle, steps: &[Step]) {
    let _ = app.emit("record://steps", steps);
}

pub fn start(app: &AppHandle, current_url: &str) {
    let state = app.state::<AppState>();
    let mut r = state.recorder.lock().unwrap();
    r.active = true;
    r.steps.clear();
    r.last_event = None;
    r.id_hints.clear();
    if recordable_url(current_url) {
        r.steps.push(nav_step(current_url));
    }
    publish(app, &r.steps);
    let _ = app.emit("record://state", json!({"recording": true}));
}

pub fn on_page_event(app: &AppHandle, browser: &Browser, session: &str, payload: &str) {
    let state = app.state::<AppState>();
    let mut r = state.recorder.lock().unwrap();
    if !r.active {
        return;
    }
    let ev: Value = match serde_json::from_str(payload) {
        Ok(v) => v,
        Err(_) => return,
    };
    // the tab the user is interacting with becomes the active tab
    if browser.active_session().as_deref() != Some(session) {
        browser.set_active_session(session);
    }
    r.last_event = Some(Instant::now());
    let fp = ev.get("fingerprint").cloned().filter(|v| v.is_object());
    let mut action = Action::default();
    let step = match ev["kind"].as_str().unwrap_or("") {
        "click" => {
            let fp = match fp {
                Some(f) => f,
                None => return,
            };
            action.kind = "click".into();
            Step { name: format!("클릭: {}", label(&fp)), action: Some(action), fingerprint: Some(fp), ..Default::default() }
        }
        "type" => {
            let fp = match fp {
                Some(f) => f,
                None => return,
            };
            action.kind = "type".into();
            action.value = ev["value"].as_str().unwrap_or("").to_string();
            action.sensitive = ev["sensitive"].as_bool().unwrap_or(false);
            if ev["idField"].as_bool() == Some(true) {
                if let Some(c) = fp["primaryCss"].as_str() {
                    r.id_hints.push(c.to_string());
                }
            }
            // Passwords never go into the scenario file: store them in the encrypted vault and record a reference.
            let host = crate::vault::host_of(ev["url"].as_str().unwrap_or(""));
            if action.sensitive && !action.value.is_empty() && crate::vault::valid_name(&host) {
                let this_css = fp["primaryCss"].as_str().unwrap_or("").to_string();
                let user_ref = format!("{{{{cred.{host}.username}}}}");
                // The text typed just before the password is taken as the login ID. In a two-step login the
                // ID was typed on the previous screen, a few steps back, into a field that looks like an ID field.
                let mut username: Option<String> = None;
                let hints = r.id_hints.clone();
                let found = r.steps.iter_mut().rev().take(8).enumerate().find(|(i, s)| {
                    kind(s) == "type" && css(s) != this_css && (*i < 3 || hints.iter().any(|h| h == css(s)))
                });
                if let Some((_, prev)) = found {
                    if let Some(a) = prev.action.as_mut() {
                        if !a.sensitive && !a.value.is_empty() && !a.value.contains("{{") {
                            username = Some(a.value.clone());
                            a.value = user_ref.clone();
                        }
                    }
                }
                let existing = crate::vault::get(&state.data_dir, &host).ok().flatten();
                let username = username.or(existing.map(|c| c.username)).unwrap_or_default();
                match crate::vault::upsert(&state.data_dir, None, &host, &host, &username, Some(&action.value)) {
                    Ok(()) => {
                        action.value = format!("{{{{cred.{host}.password}}}}");
                        action.sensitive = false;
                        let _ = app.emit("cred://changed", json!({}));
                        crate::engine::log(app, "success", format!("🔐 로그인 정보 '{host}'을(를) 암호화 저장소에 보관했습니다 (시나리오에는 참조만 기록)"));
                    }
                    Err(e) => crate::engine::log(app, "warn", format!("비밀번호를 저장소에 보관하지 못해 시나리오에 그대로 기록합니다: {e}")),
                }
            }
            let step = Step { name: format!("입력: {}", label(&fp)), action: Some(action), fingerprint: Some(fp), ..Default::default() };
            // typing implies focusing the field: drop the preceding click / earlier partial input on the same element
            if let Some(last) = r.steps.last() {
                if (kind(last) == "click" || kind(last) == "type") && css(last) == css(&step) {
                    r.steps.pop();
                }
            }
            step
        }
        "select" => {
            let fp = match fp {
                Some(f) => f,
                None => return,
            };
            action.kind = "select".into();
            action.value = ev["value"].as_str().unwrap_or("").to_string();
            Step {
                name: format!("선택: {} = {}", label(&fp), ev["label"].as_str().unwrap_or("")),
                action: Some(action),
                fingerprint: Some(fp),
                ..Default::default()
            }
        }
        "key" => {
            action.kind = "press_key".into();
            action.key = ev["key"].as_str().unwrap_or("").to_string();
            action.modifiers = ev["modifiers"].as_u64().unwrap_or(0) as u32;
            let mut name = String::from("키 입력: ");
            if action.modifiers & 2 != 0 {
                name.push_str("Ctrl+");
            }
            if action.modifiers & 1 != 0 {
                name.push_str("Alt+");
            }
            if action.modifiers & 8 != 0 {
                name.push_str("Shift+");
            }
            name.push_str(if action.key == " " { "Space" } else { &action.key });
            Step { name, action: Some(action), ..Default::default() }
        }
        "scroll" => {
            action.kind = "scroll".into();
            action.scroll_x = ev["scrollX"].as_f64().unwrap_or(0.0);
            action.scroll_y = ev["scrollY"].as_f64().unwrap_or(0.0);
            if r.steps.last().map(|l| kind(l) == "scroll").unwrap_or(false) {
                r.steps.pop();
            }
            Step { name: format!("스크롤: {}px", action.scroll_y), action: Some(action), ..Default::default() }
        }
        _ => return,
    };
    r.steps.push(step);
    publish(app, &r.steps);
}

/// Main-frame navigation. Recorded only when it was not caused by a recorded user action
/// (e.g. the user typed a URL in the address bar or used a bookmark).
pub fn on_navigated(app: &AppHandle, browser: &Browser, session: &str, url: &str) {
    let state = app.state::<AppState>();
    let mut r = state.recorder.lock().unwrap();
    if !r.active || !recordable_url(url) || browser.active_session().as_deref() != Some(session) {
        return;
    }
    let caused_by_user_action = r.last_event.map(|t| t.elapsed().as_millis() < 4000).unwrap_or(false);
    if caused_by_user_action {
        return;
    }
    if let Some(last) = r.steps.last() {
        if kind(last) == "navigate" {
            // redirect chain: keep only the final URL... unless it is the very same URL
            if last.action.as_ref().map(|a| a.url == url).unwrap_or(false) {
                return;
            }
            r.steps.pop();
        }
    }
    r.steps.push(nav_step(url));
    publish(app, &r.steps);
}
