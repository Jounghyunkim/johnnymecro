//! Raw Chrome DevTools Protocol client (browser-level WebSocket, flattened sessions).

use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;

pub const INJECT_JS: &str = include_str!("js/inject.js");
pub const BINDING: &str = "__jm_emit";

#[derive(Debug, Clone)]
pub struct CdpEvent {
    pub method: String,
    pub params: Value,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
    pub target_id: String,
    pub session_id: String,
    pub url: String,
    pub title: String,
    pub active: bool,
}

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>>;

pub struct Browser {
    tx: mpsc::UnboundedSender<Message>,
    pending: Pending,
    next_id: AtomicU64,
    pub port: u16,
    /// target_id -> page
    pages: Mutex<Vec<PageInfo>>,
    attaching: Mutex<Vec<String>>,
    pub alive: Arc<std::sync::atomic::AtomicBool>,
}

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

impl Browser {
    /// Connect to a Chrome instance exposing --remote-debugging-port.
    pub async fn connect(port: u16) -> Result<(Arc<Browser>, mpsc::UnboundedReceiver<CdpEvent>), String> {
        let http = reqwest::Client::builder().timeout(Duration::from_secs(3)).build().map_err(err)?;
        let ver: Value = http
            .get(format!("http://127.0.0.1:{port}/json/version"))
            .send()
            .await
            .map_err(|e| format!("크롬 디버깅 포트({port})에 연결할 수 없습니다: {e}"))?
            .json()
            .await
            .map_err(err)?;
        let ws_url = ver["webSocketDebuggerUrl"].as_str().ok_or("webSocketDebuggerUrl 없음")?.to_string();
        let (ws, _) = tokio_tungstenite::connect_async(ws_url.as_str()).await.map_err(|e| format!("CDP WebSocket 연결 실패: {e}"))?;
        let (mut sink, mut stream) = ws.split();

        let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
        let (ev_tx, ev_rx) = mpsc::unbounded_channel::<CdpEvent>();
        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        let alive = Arc::new(std::sync::atomic::AtomicBool::new(true));

        tokio::spawn(async move {
            while let Some(m) = rx.recv().await {
                if sink.send(m).await.is_err() {
                    break;
                }
            }
            let _ = sink.close().await;
        });

        let pending2 = pending.clone();
        let alive2 = alive.clone();
        tokio::spawn(async move {
            while let Some(Ok(msg)) = stream.next().await {
                let text = match msg {
                    Message::Text(t) => t.to_string(),
                    Message::Close(_) => break,
                    _ => continue,
                };
                let v: Value = match serde_json::from_str(&text) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                if let Some(id) = v.get("id").and_then(|x| x.as_u64()) {
                    let sender = pending2.lock().unwrap().remove(&id);
                    if let Some(s) = sender {
                        let r = if let Some(e) = v.get("error") {
                            Err(e["message"].as_str().unwrap_or("CDP error").to_string())
                        } else {
                            Ok(v.get("result").cloned().unwrap_or(Value::Null))
                        };
                        let _ = s.send(r);
                    }
                } else if let Some(method) = v.get("method").and_then(|x| x.as_str()) {
                    let _ = ev_tx.send(CdpEvent {
                        method: method.to_string(),
                        params: v.get("params").cloned().unwrap_or(Value::Null),
                        session_id: v.get("sessionId").and_then(|x| x.as_str()).map(|s| s.to_string()),
                    });
                }
            }
            alive2.store(false, Ordering::SeqCst);
            for (_, s) in pending2.lock().unwrap().drain() {
                let _ = s.send(Err("브라우저 연결이 끊어졌습니다".into()));
            }
            let _ = ev_tx.send(CdpEvent { method: "__closed".into(), params: Value::Null, session_id: None });
        });

        let b = Arc::new(Browser {
            tx,
            pending,
            next_id: AtomicU64::new(1),
            port,
            pages: Mutex::new(Vec::new()),
            attaching: Mutex::new(Vec::new()),
            alive,
        });
        b.send("Target.setDiscoverTargets", json!({"discover": true}), None).await?;
        Ok((b, ev_rx))
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    pub fn close(&self) {
        let _ = self.tx.send(Message::Close(None));
    }

    pub async fn send(&self, method: &str, params: Value, session: Option<&str>) -> Result<Value, String> {
        if !self.is_alive() {
            return Err("브라우저에 연결되어 있지 않습니다".into());
        }
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let mut msg = json!({"id": id, "method": method, "params": params});
        if let Some(s) = session {
            msg["sessionId"] = json!(s);
        }
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, tx);
        self.tx.send(Message::Text(msg.to_string().into())).map_err(|_| "브라우저 연결이 끊어졌습니다".to_string())?;
        match tokio::time::timeout(Duration::from_secs(45), rx).await {
            Ok(Ok(r)) => r.map_err(|e| format!("{method}: {e}")),
            Ok(Err(_)) => Err("브라우저 연결이 끊어졌습니다".into()),
            Err(_) => {
                self.pending.lock().unwrap().remove(&id);
                Err(format!("{method}: CDP 응답 시간 초과"))
            }
        }
    }

    // ---------- target / page bookkeeping ----------

    pub fn pages(&self) -> Vec<PageInfo> {
        self.pages.lock().unwrap().clone()
    }

    pub fn active_session(&self) -> Option<String> {
        self.pages.lock().unwrap().iter().find(|p| p.active).map(|p| p.session_id.clone())
    }

    pub fn set_active_session(&self, session: &str) -> bool {
        let mut pages = self.pages.lock().unwrap();
        if !pages.iter().any(|p| p.session_id == session) {
            return false;
        }
        for p in pages.iter_mut() {
            p.active = p.session_id == session;
        }
        true
    }

    pub fn session_of_target(&self, target_id: &str) -> Option<String> {
        self.pages.lock().unwrap().iter().find(|p| p.target_id == target_id).map(|p| p.session_id.clone())
    }

    fn is_page_target(info: &Value) -> bool {
        info["type"].as_str() == Some("page") && !info["url"].as_str().unwrap_or("").starts_with("devtools://")
    }

    /// Attach to every existing page target.
    pub async fn attach_existing(&self) -> Result<(), String> {
        let r = self.send("Target.getTargets", json!({}), None).await?;
        if let Some(list) = r["targetInfos"].as_array() {
            for info in list {
                self.maybe_attach(info).await;
            }
        }
        Ok(())
    }

    pub async fn maybe_attach(&self, info: &Value) {
        if !Self::is_page_target(info) {
            return;
        }
        let tid = info["targetId"].as_str().unwrap_or("").to_string();
        {
            let pages = self.pages.lock().unwrap();
            let mut att = self.attaching.lock().unwrap();
            if pages.iter().any(|p| p.target_id == tid) || att.contains(&tid) {
                return;
            }
            att.push(tid.clone());
        }
        let r = self.send("Target.attachToTarget", json!({"targetId": tid, "flatten": true}), None).await;
        self.attaching.lock().unwrap().retain(|t| t != &tid);
        let session = match r {
            Ok(v) => v["sessionId"].as_str().unwrap_or("").to_string(),
            Err(_) => return,
        };
        if session.is_empty() {
            return;
        }
        {
            let mut pages = self.pages.lock().unwrap();
            for p in pages.iter_mut() {
                p.active = false;
            }
            pages.push(PageInfo {
                target_id: tid,
                session_id: session.clone(),
                url: info["url"].as_str().unwrap_or("").to_string(),
                title: info["title"].as_str().unwrap_or("").to_string(),
                active: true, // newest tab becomes the active one
            });
        }
        let s = Some(session.as_str());
        let _ = self.send("Page.enable", json!({}), s).await;
        let _ = self.send("Runtime.enable", json!({}), s).await;
        let _ = self.send("Runtime.addBinding", json!({"name": BINDING}), s).await;
        let _ = self.send("Page.addScriptToEvaluateOnNewDocument", json!({"source": INJECT_JS}), s).await;
        let _ = self.send("Runtime.evaluate", json!({"expression": INJECT_JS}), s).await;
    }

    pub fn on_target_info(&self, info: &Value) {
        let tid = info["targetId"].as_str().unwrap_or("");
        let mut pages = self.pages.lock().unwrap();
        if let Some(p) = pages.iter_mut().find(|p| p.target_id == tid) {
            p.url = info["url"].as_str().unwrap_or("").to_string();
            p.title = info["title"].as_str().unwrap_or("").to_string();
        }
    }

    pub fn on_target_gone(&self, target_id: Option<&str>, session_id: Option<&str>) {
        let mut pages = self.pages.lock().unwrap();
        let was_active = pages
            .iter()
            .any(|p| p.active && (Some(p.target_id.as_str()) == target_id || Some(p.session_id.as_str()) == session_id));
        pages.retain(|p| Some(p.target_id.as_str()) != target_id && Some(p.session_id.as_str()) != session_id);
        if was_active {
            if let Some(last) = pages.last_mut() {
                last.active = true;
            }
        }
    }

    // ---------- page-level helpers (active tab) ----------

    fn sess(&self) -> Result<String, String> {
        self.active_session().ok_or_else(|| "열려 있는 브라우저 탭이 없습니다".to_string())
    }

    pub async fn page_send(&self, method: &str, params: Value) -> Result<Value, String> {
        let s = self.sess()?;
        self.send(method, params, Some(&s)).await
    }

    /// Evaluate a JS expression in the active tab, returning its value.
    pub async fn eval(&self, expr: &str) -> Result<Value, String> {
        let r = self
            .page_send("Runtime.evaluate", json!({"expression": expr, "returnByValue": true, "awaitPromise": true, "userGesture": true}))
            .await?;
        if let Some(ex) = r.get("exceptionDetails") {
            let d = ex["exception"]["description"].as_str().or(ex["text"].as_str()).unwrap_or("JS 예외");
            return Err(d.lines().next().unwrap_or(d).to_string());
        }
        Ok(r["result"]["value"].clone())
    }

    /// Call a `window.__jm` helper expression (e.g. `J.resolve(...)`); (re)injects the agent when missing.
    pub async fn jm(&self, expr: &str) -> Result<Value, String> {
        let wrapped = format!(
            "(function(){{var J=window.__jm;if(!J)return '__JM_MISSING__';return JSON.stringify({expr});}})()"
        );
        let mut v = self.eval(&wrapped).await?;
        if v.as_str() == Some("__JM_MISSING__") {
            self.eval(INJECT_JS).await?;
            v = self.eval(&wrapped).await?;
        }
        match v.as_str() {
            Some("__JM_MISSING__") => Err("이 페이지에는 스크립트를 주입할 수 없습니다".into()),
            Some(s) => serde_json::from_str(s).map_err(err),
            None => Ok(Value::Null),
        }
    }

    pub async fn url(&self) -> String {
        self.eval("location.href").await.ok().and_then(|v| v.as_str().map(|s| s.to_string())).unwrap_or_default()
    }

    /// Wait for the document to settle after an action that may navigate.
    pub async fn wait_settle(&self, max_ms: u64) {
        tokio::time::sleep(Duration::from_millis(250)).await;
        let start = std::time::Instant::now();
        loop {
            let state = self.eval("document.readyState").await;
            let elapsed = start.elapsed().as_millis() as u64;
            match state {
                Ok(v) if v.as_str() == Some("complete") => break,
                Ok(v) if v.as_str() == Some("interactive") && elapsed > max_ms / 2 => break,
                _ => {}
            }
            if elapsed > max_ms {
                break;
            }
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
    }

    /// Wait until the DOM stops changing (async rendering after an action). Used by the agent before observing.
    pub async fn settle_dom(&self, max_ms: u64) {
        let probe = "document.body ? document.body.innerText.length + '|' + document.getElementsByTagName('*').length : ''";
        let start = std::time::Instant::now();
        let mut last = String::from("?");
        loop {
            tokio::time::sleep(Duration::from_millis(300)).await;
            let cur = self.eval(probe).await.ok().and_then(|v| v.as_str().map(|s| s.to_string())).unwrap_or_default();
            if (cur == last && !cur.is_empty()) || start.elapsed().as_millis() as u64 > max_ms {
                break;
            }
            last = cur;
        }
    }

    pub async fn navigate(&self, url: &str) -> Result<(), String> {
        let r = self.page_send("Page.navigate", json!({"url": url})).await?;
        if let Some(e) = r["errorText"].as_str() {
            if !e.is_empty() && e != "net::ERR_ABORTED" {
                return Err(format!("이동 실패({url}): {e}"));
            }
        }
        self.wait_settle(20000).await;
        Ok(())
    }

    pub async fn click_at(&self, x: f64, y: f64) -> Result<(), String> {
        self.page_send("Input.dispatchMouseEvent", json!({"type": "mouseMoved", "x": x, "y": y})).await?;
        self.page_send(
            "Input.dispatchMouseEvent",
            json!({"type": "mousePressed", "x": x, "y": y, "button": "left", "buttons": 1, "clickCount": 1}),
        )
        .await?;
        self.page_send(
            "Input.dispatchMouseEvent",
            json!({"type": "mouseReleased", "x": x, "y": y, "button": "left", "buttons": 0, "clickCount": 1}),
        )
        .await?;
        Ok(())
    }

    pub async fn insert_text(&self, text: &str) -> Result<(), String> {
        self.page_send("Input.insertText", json!({"text": text})).await.map(|_| ())
    }

    /// Dispatch a real key press. `modifiers` bitmask: Alt=1, Ctrl=2, Meta=4, Shift=8.
    pub async fn press_key(&self, key: &str, modifiers: u32) -> Result<(), String> {
        let (k, code, vk, text) = key_def(key);
        let mut down = json!({
            "type": if text.is_some() && modifiers & 0b111 == 0 { "keyDown" } else { "rawKeyDown" },
            "key": k, "code": code, "windowsVirtualKeyCode": vk, "nativeVirtualKeyCode": vk, "modifiers": modifiers
        });
        if let Some(t) = &text {
            if modifiers & 0b111 == 0 {
                down["text"] = json!(t);
                down["unmodifiedText"] = json!(t);
            }
        }
        if modifiers & 2 != 0 {
            let cmd = match k.to_lowercase().as_str() {
                "a" => Some("selectAll"),
                "c" => Some("copy"),
                "v" => Some("paste"),
                "x" => Some("cut"),
                "z" => Some("undo"),
                _ => None,
            };
            if let Some(c) = cmd {
                down["commands"] = json!([c]);
            }
        }
        self.page_send("Input.dispatchKeyEvent", down).await?;
        self.page_send(
            "Input.dispatchKeyEvent",
            json!({"type": "keyUp", "key": k, "code": code, "windowsVirtualKeyCode": vk, "nativeVirtualKeyCode": vk, "modifiers": modifiers}),
        )
        .await?;
        Ok(())
    }

    /// JPEG screenshot of the viewport (base64).
    pub async fn screenshot(&self) -> Result<String, String> {
        let r = self.page_send("Page.captureScreenshot", json!({"format": "jpeg", "quality": 70})).await?;
        r["data"].as_str().map(|s| s.to_string()).ok_or_else(|| "스크린샷 실패".to_string())
    }

    pub async fn activate(&self, target_id: &str) -> Result<(), String> {
        self.send("Target.activateTarget", json!({"targetId": target_id}), None).await.map(|_| ())
    }
}

/// (key, code, windows virtual key, text)
fn key_def(key: &str) -> (String, String, u32, Option<String>) {
    let named: Option<(&str, &str, u32, Option<&str>)> = match key.to_lowercase().as_str() {
        "enter" | "return" => Some(("Enter", "Enter", 13, Some("\r"))),
        "tab" => Some(("Tab", "Tab", 9, None)),
        "escape" | "esc" => Some(("Escape", "Escape", 27, None)),
        "backspace" => Some(("Backspace", "Backspace", 8, None)),
        "delete" | "del" => Some(("Delete", "Delete", 46, None)),
        "arrowup" | "up" => Some(("ArrowUp", "ArrowUp", 38, None)),
        "arrowdown" | "down" => Some(("ArrowDown", "ArrowDown", 40, None)),
        "arrowleft" | "left" => Some(("ArrowLeft", "ArrowLeft", 37, None)),
        "arrowright" | "right" => Some(("ArrowRight", "ArrowRight", 39, None)),
        "pageup" => Some(("PageUp", "PageUp", 33, None)),
        "pagedown" => Some(("PageDown", "PageDown", 34, None)),
        "home" => Some(("Home", "Home", 36, None)),
        "end" => Some(("End", "End", 35, None)),
        " " | "space" => Some((" ", "Space", 32, Some(" "))),
        _ => None,
    };
    if let Some((k, c, vk, t)) = named {
        return (k.into(), c.into(), vk, t.map(|s| s.to_string()));
    }
    let lower = key.to_lowercase();
    if lower.starts_with('f') && lower.len() <= 3 {
        if let Ok(n) = lower[1..].parse::<u32>() {
            if (1..=12).contains(&n) {
                return (format!("F{n}"), format!("F{n}"), 111 + n, None);
            }
        }
    }
    let mut chars = key.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        if c.is_ascii_alphabetic() {
            let up = c.to_ascii_uppercase();
            return (key.to_string(), format!("Key{up}"), up as u32, Some(key.to_string()));
        }
        if c.is_ascii_digit() {
            return (key.to_string(), format!("Digit{c}"), c as u32, Some(key.to_string()));
        }
        return (key.to_string(), String::new(), 0, Some(key.to_string()));
    }
    (key.to_string(), key.to_string(), 0, None)
}

/// Locate chrome.exe (or Edge as a fallback).
pub fn find_chrome(custom: &str) -> Option<std::path::PathBuf> {
    let mut cands: Vec<std::path::PathBuf> = Vec::new();
    if !custom.trim().is_empty() {
        cands.push(custom.trim().into());
    }
    for var in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
        if let Ok(base) = std::env::var(var) {
            cands.push(std::path::Path::new(&base).join("Google\\Chrome\\Application\\chrome.exe"));
        }
    }
    for var in ["ProgramFiles(x86)", "ProgramFiles"] {
        if let Ok(base) = std::env::var(var) {
            cands.push(std::path::Path::new(&base).join("Microsoft\\Edge\\Application\\msedge.exe"));
        }
    }
    cands.into_iter().find(|p| p.is_file())
}
