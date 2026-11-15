//! Local Ollama client (Module E): model discovery + structured-output chat.

use serde_json::{json, Value};
use std::time::Duration;

#[derive(Clone)]
pub struct Ollama {
    pub base: String,
    http: reqwest::Client,
}

impl Ollama {
    pub fn new(base: &str) -> Self {
        let base = base.trim().trim_end_matches('/').to_string();
        Ollama {
            base: if base.is_empty() { "http://127.0.0.1:11434".into() } else { base },
            http: reqwest::Client::builder().connect_timeout(Duration::from_secs(4)).build().unwrap_or_default(),
        }
    }

    /// GET /api/tags
    pub async fn models(&self) -> Result<Value, String> {
        let r = self
            .http
            .get(format!("{}/api/tags", self.base))
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| format!("Ollama에 연결할 수 없습니다 ({}): {e}", self.base))?;
        let v: Value = r.json().await.map_err(|e| e.to_string())?;
        let list: Vec<Value> = v["models"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|m| {
                        let caps = m["capabilities"].as_array().cloned().unwrap_or_default();
                        let name = m["name"].as_str().unwrap_or("").to_string();
                        let lname = name.to_lowercase();
                        let vision = caps.iter().any(|c| c.as_str() == Some("vision"))
                            || ["vision", "-vl", "llava", "minicpm-v", "moondream", "gemma3", "gemma4"].iter().any(|k| lname.contains(k));
                        json!({
                            "name": name,
                            "size": m["size"],
                            "parameterSize": m["details"]["parameter_size"],
                            "vision": vision,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(Value::Array(list))
    }

    /// POST /api/chat with a JSON-schema `format`. Returns the parsed JSON object.
    pub async fn chat_json(&self, model: &str, system: &str, user: &str, image_b64: Option<&str>, schema: &Value) -> Result<Value, String> {
        if model.trim().is_empty() {
            return Err("Ollama 모델이 선택되지 않았습니다".into());
        }
        let mut user_msg = json!({"role": "user", "content": user});
        if let Some(img) = image_b64 {
            user_msg["images"] = json!([img]);
        }
        let mut body = json!({
            "model": model,
            "messages": [{"role": "system", "content": system}, user_msg],
            "stream": false,
            "format": schema,
            "keep_alive": "30m",
            "think": false,
            "options": {"temperature": 0.1, "num_ctx": 16384}
        });
        let mut last_err = String::new();
        for attempt in 0..3 {
            let resp = self
                .http
                .post(format!("{}/api/chat", self.base))
                .timeout(Duration::from_secs(300))
                .json(&body)
                .send()
                .await
                .map_err(|e| format!("Ollama 요청 실패: {e}"))?;
            let status = resp.status();
            let text = resp.text().await.map_err(|e| e.to_string())?;
            if !status.is_success() {
                // models without thinking support reject the `think` parameter
                if text.contains("think") && body.get("think").is_some() {
                    body.as_object_mut().unwrap().remove("think");
                    continue;
                }
                return Err(format!("Ollama 오류 {status}: {}", text.chars().take(300).collect::<String>()));
            }
            let v: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            let content = v["message"]["content"].as_str().unwrap_or("").trim().to_string();
            // optional prompt/response dump for troubleshooting: set JM_DEBUG_DIR to a folder
            if let Ok(dir) = std::env::var("JM_DEBUG_DIR") {
                use std::io::Write;
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(std::path::Path::new(&dir).join("llm.log")) {
                    let _ = writeln!(f, "===== {} [{model}] =====\n--- system ---\n{system}\n--- user ---\n{user}\n--- response ---\n{content}\n", chrono::Local::now().format("%H:%M:%S"));
                }
            }
            match parse_json_loose(&content) {
                Some(j) => return Ok(j),
                None => last_err = format!("LLM이 JSON이 아닌 응답을 반환했습니다(시도 {}): {}", attempt + 1, content.chars().take(200).collect::<String>()),
            }
        }
        Err(last_err)
    }
}

fn parse_json_loose(s: &str) -> Option<Value> {
    if let Ok(v) = serde_json::from_str::<Value>(s) {
        return Some(v);
    }
    let start = s.find(|c| c == '{' || c == '[')?;
    let end = s.rfind(|c| c == '}' || c == ']')?;
    if end <= start {
        return None;
    }
    serde_json::from_str(&s[start..=end]).ok()
}

/// Structured output schema for the agent action (spec Module E-2).
///
/// `target_index` is constrained to the indexes that actually exist on the current page (0 = no target),
/// so a small local model cannot hallucinate an element. Property order matters: `thought` is generated first.
pub fn agent_schema(element_count: u64) -> Value {
    json!({
        "type": "object",
        "properties": {
            "thought": {"type": "string"},
            "goal_status": {"type": "string", "enum": ["IN_PROGRESS", "COMPLETED", "FAILED", "NEED_USER_HELP"]},
            "action": {
                "type": "object",
                "properties": {
                    "type": {"type": "string", "enum": ["CLICK", "TYPE", "PRESS_KEY", "SCROLL", "WAIT", "EXTRACT", "NAVIGATE", "NONE"]},
                    "target_index": index_enum(0, element_count),
                    "input_text": {"type": "string"},
                    "scroll_direction": {"type": "string", "enum": ["DOWN", "UP"]}
                },
                "required": ["type", "target_index", "input_text", "scroll_direction"]
            }
        },
        "required": ["thought", "goal_status", "action"]
    })
}

/// Integer restricted to `none_value` or 1..=count.
pub fn index_enum(none_value: i64, count: u64) -> Value {
    let mut values: Vec<i64> = vec![none_value];
    values.extend(1..=(count.min(400) as i64));
    json!({"type": "integer", "enum": values})
}
