//! Credential vault: login IDs / passwords encrypted at rest with Windows DPAPI.
//!
//! The file can only be decrypted by the same Windows user account on the same machine.
//! Passwords never leave the backend: they are not returned to the UI, not written to scenario
//! files or logs, and never included in LLM prompts.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Credential {
    /// reference name used in scenarios: {{cred.<name>.username}} / {{cred.<name>.password}}
    pub name: String,
    /// site host this login belongs to (e.g. "example.com")
    pub site: String,
    pub username: String,
    pub password: String,
    pub updated_at: String,
}

/// What the user typed into the login dialog.
pub struct CredAnswer {
    pub username: String,
    pub password: String,
    pub save: bool,
}

static LOCK: Mutex<()> = Mutex::new(());
const ENTROPY: &[u8] = b"JohnnyMecro.vault.v1";

fn vault_path(data_dir: &Path) -> PathBuf {
    data_dir.join("credentials.dat")
}

/// "https://www.Example.com/login?x" -> "example.com"
pub fn host_of(site: &str) -> String {
    let s = site.trim().to_lowercase();
    let s = s.split("://").last().unwrap_or("");
    let s = s.split(['/', '?', '#']).next().unwrap_or("");
    let s = s.rsplit('@').next().unwrap_or("");
    let s = s.split(':').next().unwrap_or("");
    s.trim_start_matches("www.").to_string()
}

pub fn valid_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 80 && name.chars().all(|c| c.is_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

pub fn load(data_dir: &Path) -> Result<Vec<Credential>, String> {
    let _g = LOCK.lock().unwrap();
    load_unlocked(data_dir)
}

fn load_unlocked(data_dir: &Path) -> Result<Vec<Credential>, String> {
    let path = vault_path(data_dir);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let enc = std::fs::read(&path).map_err(|e| format!("로그인 정보 저장소를 읽을 수 없습니다: {e}"))?;
    let plain = dpapi::unprotect(&enc, ENTROPY)
        .map_err(|e| format!("로그인 정보 저장소를 복호화할 수 없습니다(다른 Windows 계정/PC에서 만든 파일일 수 있습니다): {e}"))?;
    serde_json::from_slice(&plain).map_err(|e| format!("로그인 정보 저장소가 손상되었습니다: {e}"))
}

fn save_unlocked(data_dir: &Path, list: &[Credential]) -> Result<(), String> {
    std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
    let mut plain = serde_json::to_vec(list).map_err(|e| e.to_string())?;
    let enc = dpapi::protect(&plain, ENTROPY);
    plain.iter_mut().for_each(|b| *b = 0);
    let enc = enc?;
    let path = vault_path(data_dir);
    let tmp = path.with_extension("dat.tmp");
    std::fs::write(&tmp, enc).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

/// Insert or update. `password = None` keeps the stored password; `original` renames an existing entry.
pub fn upsert(data_dir: &Path, original: Option<&str>, name: &str, site: &str, username: &str, password: Option<&str>) -> Result<(), String> {
    let name = name.trim();
    if !valid_name(name) {
        return Err("이름에는 문자, 숫자, '.', '-', '_' 만 사용할 수 있습니다".into());
    }
    let _g = LOCK.lock().unwrap();
    let mut list = load_unlocked(data_dir)?;
    let key = original.unwrap_or(name);
    if key != name && list.iter().any(|c| c.name == name) {
        return Err(format!("'{name}' 이름의 로그인 정보가 이미 있습니다"));
    }
    let now = crate::scenario::now();
    match list.iter_mut().find(|c| c.name == key) {
        Some(c) => {
            c.name = name.to_string();
            c.site = host_of(site);
            c.username = username.to_string();
            if let Some(p) = password {
                c.password = p.to_string();
            }
            c.updated_at = now;
        }
        None => list.push(Credential {
            name: name.to_string(),
            site: host_of(site),
            username: username.to_string(),
            password: password.unwrap_or("").to_string(),
            updated_at: now,
        }),
    }
    save_unlocked(data_dir, &list)
}

pub fn delete(data_dir: &Path, name: &str) -> Result<(), String> {
    let _g = LOCK.lock().unwrap();
    let mut list = load_unlocked(data_dir)?;
    list.retain(|c| c.name != name);
    save_unlocked(data_dir, &list)
}

pub fn get(data_dir: &Path, name: &str) -> Result<Option<Credential>, String> {
    Ok(load(data_dir)?.into_iter().find(|c| c.name == name))
}

/// Credential for a page host: exact site match first, then parent-domain match.
pub fn find_for_host(data_dir: &Path, host: &str) -> Result<Option<Credential>, String> {
    let host = host_of(host);
    if host.is_empty() {
        return Ok(None);
    }
    let list = load(data_dir)?;
    if let Some(c) = list.iter().find(|c| c.site == host || c.name == host) {
        return Ok(Some(c.clone()));
    }
    Ok(list.into_iter().find(|c| !c.site.is_empty() && host.ends_with(&format!(".{}", c.site))))
}

mod dpapi {
    use std::ffi::c_void;

    #[repr(C)]
    struct Blob {
        cb: u32,
        pb: *mut u8,
    }

    #[link(name = "crypt32")]
    extern "system" {
        fn CryptProtectData(input: *const Blob, descr: *const u16, entropy: *const Blob, reserved: *const c_void, prompt: *const c_void, flags: u32, output: *mut Blob) -> i32;
        fn CryptUnprotectData(input: *const Blob, descr: *mut *mut u16, entropy: *const Blob, reserved: *const c_void, prompt: *const c_void, flags: u32, output: *mut Blob) -> i32;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn LocalFree(mem: *mut c_void) -> *mut c_void;
    }

    const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x1;

    fn take(out: Blob) -> Vec<u8> {
        // SAFETY: on success DPAPI returns a LocalAlloc'd buffer of `cb` bytes that we own and must free.
        unsafe {
            let v = std::slice::from_raw_parts(out.pb, out.cb as usize).to_vec();
            std::ptr::write_bytes(out.pb, 0, out.cb as usize);
            LocalFree(out.pb as *mut c_void);
            v
        }
    }

    pub fn protect(data: &[u8], entropy: &[u8]) -> Result<Vec<u8>, String> {
        let input = Blob { cb: data.len() as u32, pb: data.as_ptr() as *mut u8 };
        let ent = Blob { cb: entropy.len() as u32, pb: entropy.as_ptr() as *mut u8 };
        let mut out = Blob { cb: 0, pb: std::ptr::null_mut() };
        // SAFETY: all blobs point to live buffers of the stated length for the duration of the call.
        let ok = unsafe { CryptProtectData(&input, std::ptr::null(), &ent, std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut out) };
        if ok == 0 {
            return Err(format!("DPAPI 암호화 실패: {}", std::io::Error::last_os_error()));
        }
        Ok(take(out))
    }

    pub fn unprotect(data: &[u8], entropy: &[u8]) -> Result<Vec<u8>, String> {
        let input = Blob { cb: data.len() as u32, pb: data.as_ptr() as *mut u8 };
        let ent = Blob { cb: entropy.len() as u32, pb: entropy.as_ptr() as *mut u8 };
        let mut out = Blob { cb: 0, pb: std::ptr::null_mut() };
        // SAFETY: see `protect`.
        let ok = unsafe { CryptUnprotectData(&input, std::ptr::null_mut(), &ent, std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut out) };
        if ok == 0 {
            return Err(format!("{}", std::io::Error::last_os_error()));
        }
        Ok(take(out))
    }
}
