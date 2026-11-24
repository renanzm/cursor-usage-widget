//! Uso do Codex (OpenAI) lido dos logs locais (`~/.codex/sessions/**/rollout-*.jsonl[.zst]`).
//! O custo em US$ é uma estimativa: tokens x tabela de preços; os limites vêm de `rate_limits`.
use crate::{
    claude::{min_mtime, period_start_ms, summarize, to_ms},
    config,
    settings::Settings,
    usage::Snapshot,
};
use chrono::DateTime;
use serde_json::Value;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    sync::Mutex,
    time::SystemTime,
};

/// Eventos de custo (instante ms, centavos) e a última leitura de limites (instante ms, `rate_limits`).
#[derive(Default, Clone)]
struct Parsed {
    events: Vec<(i64, f64)>,
    limits: Option<(i64, Value)>,
}

struct Cached {
    mtime: SystemTime,
    len: u64,
    parsed: Parsed,
}

static CACHE: Mutex<Option<HashMap<PathBuf, Cached>>> = Mutex::new(None);

/// `$CODEX_HOME` ou `~/.codex`.
fn home_dir() -> PathBuf {
    if let Some(p) = config::setting("CODEX_HOME") {
        return PathBuf::from(p);
    }
    let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).unwrap_or_default();
    PathBuf::from(home).join(".codex")
}

/// Pasta de sessões do Codex existe nesta máquina?
pub fn available() -> bool {
    home_dir().join("sessions").is_dir()
}

/// (entrada, entrada em cache, saída) em US$ por milhão de tokens.
/// Ajustáveis por `PRICE_CODEX_IN` / `PRICE_CODEX_CACHED` / `PRICE_CODEX_OUT`.
fn price(model: &str) -> (f64, f64, f64) {
    let m = model.to_ascii_lowercase();
    let (i, o) = if m.contains("nano") {
        (0.05, 0.4)
    } else if m.contains("mini") {
        (0.25, 2.0)
    } else {
        (1.25, 10.0)
    };
    let i = config::setting_f64("PRICE_CODEX_IN", i);
    (i, config::setting_f64("PRICE_CODEX_CACHED", i * 0.1), config::setting_f64("PRICE_CODEX_OUT", o))
}

fn ts_ms(v: &Value) -> Option<i64> {
    DateTime::parse_from_rfc3339(v["timestamp"].as_str()?).ok().map(|t| t.timestamp_millis())
}

fn parse(text: &str) -> Parsed {
    let mut out = Parsed::default();
    let mut model = String::new();
    let mut last_total: Option<u64> = None;
    for line in text.lines() {
        // Corte barato: só interessam o contexto do turno (modelo) e as contagens de tokens.
        if !line.contains("\"turn_context\"") && !line.contains("\"token_count\"") {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        let p = &v["payload"];
        if v["type"] == "turn_context" {
            if let Some(m) = p["model"].as_str() {
                model = m.to_owned();
            }
            continue;
        }
        if p["type"] != "token_count" {
            continue;
        }
        let Some(ts) = ts_ms(&v) else { continue };
        if p["rate_limits"].is_object() {
            out.limits = Some((ts, p["rate_limits"].clone()));
        }
        let info = &p["info"];
        // O mesmo total pode ser reemitido sem tokens novos: conta só quando o acumulado muda.
        let total = info["total_token_usage"]["total_tokens"].as_u64();
        if total.is_none() || total == last_total {
            continue;
        }
        last_total = total;
        let u = &info["last_token_usage"];
        let n = |k: &str| u[k].as_f64().unwrap_or(0.0);
        let (input, cached, output) = (n("input_tokens"), n("cached_input_tokens"), n("output_tokens"));
        let (pi, pc, po) = price(&model);
        let usd = ((input - cached).max(0.0) * pi + cached * pc + output * po) / 1e6;
        out.events.push((ts, usd * 100.0));
    }
    out
}

fn read_file(path: &Path) -> Option<String> {
    let mut f = File::open(path).ok()?;
    let mut bytes = Vec::new();
    if path.extension().is_some_and(|x| x == "zst") {
        ruzstd::decoding::StreamingDecoder::new(&mut f).ok()?.read_to_end(&mut bytes).ok()?;
    } else {
        f.read_to_end(&mut bytes).ok()?;
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn collect(dir: &Path, min: SystemTime, out: &mut Vec<(PathBuf, SystemTime, u64)>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let Ok(md) = e.metadata() else { continue };
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if md.is_dir() {
            collect(&p, min, out);
        } else if name.ends_with(".jsonl") || name.ends_with(".jsonl.zst") {
            let mtime = md.modified().unwrap_or(SystemTime::now());
            if mtime >= min {
                out.push((p, mtime, md.len()));
            }
        }
    }
}

fn window_label(minutes: f64) -> String {
    match minutes.round() as i64 {
        // "week" é traduzido pela interface.
        10080 => "week".into(),
        m if m < 1440 => format!("{}h", (m as f64 / 60.0).round()),
        m => format!("{}d", (m as f64 / 1440.0).round()),
    }
}

/// `primary`/`secondary` → janela curta no slot "5h", longa no slot "semana".
fn apply_limits(snap: &mut Snapshot, at_ms: i64, rl: &Value) {
    let mut wins: Vec<(f64, f64, Option<i64>)> = ["primary", "secondary"]
        .iter()
        .filter_map(|k| {
            let w = &rl[*k];
            let pct = w["used_percent"].as_f64()?;
            let minutes = w["window_minutes"].as_f64().unwrap_or(0.0);
            let reset = to_ms(&w["resets_at"])
                .or_else(|| w["resets_in_seconds"].as_f64().map(|s| at_ms + (s * 1000.0) as i64));
            Some((minutes, pct, reset))
        })
        .collect();
    if wins.is_empty() {
        return;
    }
    wins.sort_by(|a, b| a.0.total_cmp(&b.0));
    let long = (wins.len() > 1 || wins[0].0 > 1440.0).then(|| wins.pop()).flatten();
    if let Some((m, pct, reset)) = wins.first().copied() {
        snap.limit_5h_pct = Some(pct);
        snap.limit_5h_resets_ms = reset;
        snap.limit_5h_label = Some(window_label(m));
    }
    if let Some((m, pct, reset)) = long {
        snap.limit_7d_pct = Some(pct);
        snap.limit_7d_resets_ms = reset;
        snap.limit_7d_label = Some(window_label(m));
    }
    snap.limits_updated_ms = Some(at_ms.max(0) as u64);
}

fn compute(settings: &Settings) -> Result<Snapshot, String> {
    let root = home_dir();
    let sessions = root.join("sessions");
    if !sessions.is_dir() {
        return Err(format!("{}: {}", crate::i18n::tr("Pasta não encontrada", "Folder not found"), sessions.display()));
    }
    let from = period_start_ms();
    let mut all = Vec::new();
    collect(&sessions, SystemTime::UNIX_EPOCH, &mut all);
    collect(&root.join("archived_sessions"), SystemTime::UNIX_EPOCH, &mut all);
    // Do período + a sessão mais recente (os limites valem até o reset, mesmo de um mês anterior).
    let newest = all.iter().map(|f| f.1).max();
    let min = min_mtime();
    let files: Vec<_> = all.into_iter().filter(|f| f.1 >= min || Some(f.1) == newest).collect();

    let mut guard = CACHE.lock().unwrap();
    let cache = guard.get_or_insert_with(HashMap::new);
    let mut events = Vec::new();
    let mut limits: Option<(i64, Value)> = None;
    for (path, mtime, len) in files {
        let fresh = cache.get(&path).is_some_and(|c| c.mtime == mtime && c.len == len);
        if !fresh {
            // Arquivos .zst não crescem por acréscimo: relê inteiro quando muda.
            let parsed = read_file(&path).map(|t| parse(&t)).unwrap_or_default();
            cache.insert(path.clone(), Cached { mtime, len, parsed });
        }
        let p = &cache[&path].parsed;
        events.extend(p.events.iter().copied().filter(|e| e.0 >= from));
        if let Some(l) = &p.limits {
            if limits.as_ref().map_or(true, |x| l.0 > x.0) {
                limits = Some(l.clone());
            }
        }
    }
    drop(guard);

    let mut snap = summarize(events.into_iter(), settings);
    if let Some((at, rl)) = limits {
        apply_limits(&mut snap, at, &rl);
    }
    Ok(snap)
}

pub async fn fetch(settings: &Settings) -> Result<Snapshot, String> {
    let s = settings.clone();
    tokio::task::spawn_blocking(move || compute(&s)).await.map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = r#"{"timestamp":"2026-11-24T10:00:00Z","type":"turn_context","payload":{"model":"gpt-5-codex"}}
{"timestamp":"2026-11-24T10:00:01Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"total_tokens":1100},"last_token_usage":{"input_tokens":1000000,"cached_input_tokens":0,"output_tokens":100000}},"rate_limits":{"primary":{"used_percent":40.0,"window_minutes":300,"resets_at":1795498200},"secondary":{"used_percent":12.5,"window_minutes":10080,"resets_in_seconds":60}}}}
{"timestamp":"2026-11-24T10:00:02Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"total_tokens":1100},"last_token_usage":{"input_tokens":1000000,"cached_input_tokens":0,"output_tokens":100000}}}}
{"timestamp":"2026-11-24T10:00:03Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"total_tokens":2200},"last_token_usage":{"input_tokens":1000000,"cached_input_tokens":1000000,"output_tokens":0}}}}"#;

    #[test]
    fn parses_cost_and_skips_repeated_totals() {
        let p = parse(LOG);
        assert_eq!(p.events.len(), 2);
        // 1M entrada x 1,25 + 100k saída x 10 = US$ 2,25; 1M em cache x 0,125 = US$ 0,125
        assert!((p.events[0].1 - 225.0).abs() < 1e-6);
        assert!((p.events[1].1 - 12.5).abs() < 1e-6);
    }

    #[test]
    fn maps_windows_to_short_and_long() {
        let p = parse(LOG);
        let (at, rl) = p.limits.unwrap();
        let mut s = Snapshot::empty("ok", None, &Settings::default());
        apply_limits(&mut s, at, &rl);
        assert_eq!(s.limit_5h_pct, Some(40.0));
        assert_eq!(s.limit_5h_resets_ms, Some(1_795_498_200_000));
        assert_eq!(s.limit_5h_label.as_deref(), Some("5h"));
        assert_eq!(s.limit_7d_pct, Some(12.5));
        assert_eq!(s.limit_7d_resets_ms, Some(at + 60_000));
        assert_eq!(s.limit_7d_label.as_deref(), Some("week"));
    }

    /// `CODEX_LOG=<arquivo .jsonl.zst> cargo test real_log -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_log() {
        let path = PathBuf::from(std::env::var("CODEX_LOG").unwrap());
        let p = parse(&read_file(&path).expect("leitura"));
        let usd: f64 = p.events.iter().map(|e| e.1).sum::<f64>() / 100.0;
        println!("eventos={} usd={usd:.4} limites={:?}", p.events.len(), p.limits);
        assert!(!p.events.is_empty());
    }

    #[test]
    fn single_long_window_goes_to_long_slot() {
        let rl: Value = serde_json::from_str(r#"{"primary":{"used_percent":3.0,"window_minutes":43200,"resets_at":1}}"#).unwrap();
        let mut s = Snapshot::empty("ok", None, &Settings::default());
        apply_limits(&mut s, 0, &rl);
        assert_eq!(s.limit_5h_pct, None);
        assert_eq!(s.limit_7d_pct, Some(3.0));
        assert_eq!(s.limit_7d_label.as_deref(), Some("30d"));
    }
}
