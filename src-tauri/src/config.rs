//! Configuração: cookie da sessão (Credential Manager > env > .env) e ajustes numéricos.
use std::{env, fs, path::PathBuf};

const SERVICE: &str = "cursor-usage-widget";
const ACCOUNT: &str = "session-cookie";

/// Normaliza o valor colado pelo usuário para o formato esperado do cookie.
pub fn normalize(raw: &str) -> Option<String> {
    let mut v = raw
        .trim()
        .trim_matches(|c| c == '"' || c == '\'')
        .trim()
        .to_string();
    if let Some(rest) = v.strip_prefix("WorkosCursorSessionToken=") {
        v = rest.to_string();
    }
    if let Some(i) = v.find(';') {
        v.truncate(i);
    }
    v = v.replace("::", "%3A%3A");
    // Cópia pelo DevTools às vezes perde o primeiro caractere ("user_" -> "ser_").
    if v.starts_with("ser_") {
        v.insert(0, 'u');
    }
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}

fn dotenv_paths() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut roots = Vec::new();
    if let Ok(cwd) = env::current_dir() {
        roots.push(cwd);
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(dir) = exe.parent() {
            roots.push(dir.to_path_buf());
        }
    }
    for root in roots {
        for dir in root.ancestors().take(6) {
            out.push(dir.join(".env"));
        }
    }
    out
}

fn dotenv_var(key: &str) -> Option<String> {
    for path in dotenv_paths() {
        let Ok(text) = fs::read_to_string(&path) else { continue };
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                if k.trim() == key {
                    return Some(v.trim().to_string());
                }
            }
        }
    }
    None
}

pub fn setting(key: &str) -> Option<String> {
    env::var(key).ok().filter(|v| !v.is_empty()).or_else(|| dotenv_var(key))
}

pub fn setting_f64(key: &str, default: f64) -> f64 {
    setting(key).and_then(|v| v.parse().ok()).unwrap_or(default)
}

pub fn load_cookie() -> Option<String> {
    if let Ok(entry) = keyring::Entry::new(SERVICE, ACCOUNT) {
        if let Ok(v) = entry.get_password() {
            if let Some(n) = normalize(&v) {
                return Some(n);
            }
        }
    }
    // Veio do env/.env: guarda no Credential Manager para o executável funcionar de qualquer pasta.
    let cookie = setting("CURSOR_COOKIE").and_then(|v| normalize(&v))?;
    let _ = save_cookie(&cookie);
    Some(cookie)
}

pub fn save_cookie(cookie: &str) -> Result<(), String> {
    keyring::Entry::new(SERVICE, ACCOUNT)
        .and_then(|e| e.set_password(cookie))
        .map_err(|e| format!("Falha ao salvar no Credential Manager: {e}"))
}
