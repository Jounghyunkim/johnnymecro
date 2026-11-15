//! Scenario schema (Module B), settings, and file persistence.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const T_ACTION: &str = "DETERMINISTIC_ACTION";
pub const T_CONDITION: &str = "LLM_CONDITION";
pub const T_EXTRACT: &str = "LLM_EXTRACT";
pub const T_GOAL: &str = "AI_GOAL_STEP";
pub const T_LOOP: &str = "LOOP";

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Action {
    /// click | type | press_key | scroll | wait | navigate | select
    pub kind: String,
    pub value: String,
    pub key: String,
    pub modifiers: u32,
    pub url: String,
    pub ms: u64,
    pub scroll_x: f64,
    pub scroll_y: f64,
    pub sensitive: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct LoopCfg {
    /// count | list | while
    pub mode: String,
    pub count: u32,
    pub list_var: String,
    pub item_var: String,
    /// natural-language condition evaluated by the LLM before each iteration (mode = while)
    pub condition: String,
    pub max_iterations: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Step {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
    pub enabled: bool,
    // DETERMINISTIC_ACTION
    pub action: Option<Action>,
    pub fingerprint: Option<Value>,
    pub timeout_ms: u64,
    pub retries: u32,
    pub llm_fallback: bool,
    /// stop | continue
    pub on_error: String,
    // LLM_CONDITION
    pub question: String,
    pub options: Vec<String>,
    pub branches: BTreeMap<String, Vec<Step>>,
    // LLM_EXTRACT
    pub instruction: String,
    pub schema: Option<Value>,
    pub output_var: String,
    // AI_GOAL_STEP
    pub goal: String,
    pub max_steps: u32,
    pub compile_to_macro: bool,
    // LOOP
    #[serde(rename = "loop")]
    pub loop_cfg: Option<LoopCfg>,
    pub children: Vec<Step>,
}

impl Default for Step {
    fn default() -> Self {
        Step {
            id: new_id(),
            kind: T_ACTION.into(),
            name: String::new(),
            enabled: true,
            action: None,
            fingerprint: None,
            timeout_ms: 0,
            retries: 0,
            llm_fallback: true,
            on_error: "stop".into(),
            question: String::new(),
            options: Vec::new(),
            branches: BTreeMap::new(),
            instruction: String::new(),
            schema: None,
            output_var: String::new(),
            goal: String::new(),
            max_steps: 0,
            compile_to_macro: false,
            loop_cfg: None,
            children: Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Scenario {
    pub id: String,
    pub name: String,
    pub description: String,
    pub created_at: String,
    pub updated_at: String,
    pub variables: Map<String, Value>,
    pub steps: Vec<Step>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub debug_port: u16,
    pub chrome_path: String,
    /// empty = dedicated profile inside the app data dir
    pub user_data_dir: String,
    pub ollama_url: String,
    pub text_model: String,
    pub vision_model: String,
    /// never | fallback | always
    pub vision_mode: String,
    pub default_timeout_ms: u64,
    pub step_delay_ms: u64,
    pub max_agent_steps: u32,
    pub confirm_sensitive: bool,
    pub use_planner: bool,
    pub use_validator: bool,
    /// comma separated; empty = any domain allowed for agent NAVIGATE
    pub allowed_domains: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            debug_port: 9222,
            chrome_path: String::new(),
            user_data_dir: String::new(),
            ollama_url: "http://127.0.0.1:11434".into(),
            text_model: String::new(),
            vision_model: String::new(),
            vision_mode: "fallback".into(),
            default_timeout_ms: 5000,
            step_delay_ms: 300,
            max_agent_steps: 25,
            confirm_sensitive: true,
            use_planner: true,
            use_validator: true,
            allowed_domains: String::new(),
        }
    }
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..12].to_string()
}

pub fn now() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

fn safe_id(id: &str) -> Result<(), String> {
    if !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        Ok(())
    } else {
        Err("잘못된 시나리오 ID".into())
    }
}

pub fn scenario_path(dir: &Path, id: &str) -> Result<PathBuf, String> {
    safe_id(id)?;
    Ok(dir.join(format!("{id}.json")))
}

pub fn save_scenario(dir: &Path, sc: &mut Scenario) -> Result<(), String> {
    if sc.id.is_empty() {
        sc.id = new_id();
    }
    if sc.created_at.is_empty() {
        sc.created_at = now();
    }
    sc.updated_at = now();
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = scenario_path(dir, &sc.id)?;
    let tmp = path.with_extension("json.tmp");
    let data = serde_json::to_string_pretty(sc).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, data).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

pub fn load_scenario(dir: &Path, id: &str) -> Result<Scenario, String> {
    let data = std::fs::read_to_string(scenario_path(dir, id)?).map_err(|e| e.to_string())?;
    serde_json::from_str(data.trim_start_matches('\u{feff}')).map_err(|e| format!("시나리오 파싱 실패: {e}"))
}

pub fn list_scenarios(dir: &Path) -> Vec<Scenario> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("json") {
                continue;
            }
            if let Some(id) = p.file_stem().and_then(|x| x.to_str()) {
                if let Ok(mut sc) = load_scenario(dir, id) {
                    sc.id = id.to_string();
                    out.push(sc);
                }
            }
        }
    }
    out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    out
}

// ---------- variable binding: {{name}} / {{name.path.0}} ----------

fn lookup<'a>(vars: &'a Map<String, Value>, path: &str) -> Option<&'a Value> {
    let mut parts = path.split('.');
    let mut cur = vars.get(parts.next()?.trim())?;
    for p in parts {
        let p = p.trim();
        cur = match cur {
            Value::Object(m) => m.get(p)?,
            Value::Array(a) => a.get(p.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(cur)
}

pub fn value_to_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

pub fn substitute(s: &str, vars: &Map<String, Value>) -> String {
    substitute_with(s, |name| lookup(vars, name).map(value_to_text))
}

/// Names of credentials referenced as {{cred.<name>.username|password}}.
pub fn cred_refs(s: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    substitute_with(s, |token| {
        if let Some((name, _)) = parse_cred_token(token) {
            if !out.contains(&name) {
                out.push(name);
            }
        }
        None
    });
    out
}

/// "cred.example.com.password" -> ("example.com", "password")
pub fn parse_cred_token(token: &str) -> Option<(String, String)> {
    let rest = token.trim().strip_prefix("cred.")?;
    let (name, field) = rest.rsplit_once('.')?;
    if name.is_empty() || !(field == "username" || field == "password") {
        return None;
    }
    Some((name.to_string(), field.to_string()))
}

/// Single-pass {{token}} replacement. Replaced text is never re-scanned, so substituted values
/// (page-extracted data, passwords) cannot smuggle in further placeholders.
pub fn substitute_with(s: &str, mut resolve: impl FnMut(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find("}}") {
            Some(end) => {
                let name = &after[..end];
                match resolve(name) {
                    Some(v) => out.push_str(&v),
                    None => {
                        out.push_str("{{");
                        out.push_str(name);
                        out.push_str("}}");
                    }
                }
                rest = &after[end + 2..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}
