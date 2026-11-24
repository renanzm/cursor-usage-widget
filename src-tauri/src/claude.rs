//! Uso do Claude Code lido dos logs locais (`~/.claude/projects/**/*.jsonl`).
//! O custo em US$ é uma estimativa: tokens x tabela de preços por modelo.
use crate::{
    config,
    settings::Settings,
    usage::{level_of, now_ms, Snapshot},
};
use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, TimeZone};
use serde_json::Value;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{BufRead, BufReader, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::Mutex,
    time::SystemTime,
};

#[derive(Clone)]
struct Ev {
    ts_ms: i64,
    id: String,
    cents: f64,
}

#[derive(Default)]
struct FileCache {
    offset: u64,
    events: Vec<Ev>,
}

static CACHE: Mutex<Option<HashMap<PathBuf, FileCache>>> = Mutex::new(None);

/// (entrada, saída) em US$ por milhão de tokens. Cache: escrita 1,25x e leitura 0,1x da entrada.
/// Ajustáveis por `PRICE_<FAMILIA>_IN` / `PRICE_<FAMILIA>_OUT` (OPUS, SONNET, HAIKU).
fn price(model: &str) -> (f64, f64) {
    let m = model.to_lowercase();
    let (fam, din, dout) = if m.contains("haiku") {
        ("HAIKU", 1.0, 5.0)
    } else if m.contains("sonnet") {
        ("SONNET", 3.0, 15.0)
    } else {
        ("OPUS", 5.0, 25.0) // opus, fable e desconhecidos
    };
    (
        config::setting_f64(&format!("PRICE_{fam}_IN"), din),
        config::setting_f64(&format!("PRICE_{fam}_OUT"), dout),
    )
}

fn parse_line(line: &str) -> Option<Ev> {
    // Filtro barato antes de decodificar o JSON (a maioria das linhas não é de uso).
    if !line.contains("\"usage\"") || !line.contains("\"assistant\"") {
        return None;
    }
    let v: Value = serde_json::from_str(line).ok()?;
    if v["type"].as_str() != Some("assistant") {
        return None;
    }
    let msg = &v["message"];
    let u = &msg["usage"];
    let model = msg["model"].as_str().unwrap_or("");
    if model.starts_with('<') {
        return None; // mensagens sintéticas
    }
    let ts_ms = DateTime::parse_from_rfc3339(v["timestamp"].as_str()?).ok()?.timestamp_millis();
    let n = |k: &str| u[k].as_f64().unwrap_or(0.0);
    let (pin, pout) = price(model);
    let usd = (n("input_tokens") * pin
        + n("cache_creation_input_tokens") * pin * 1.25
        + n("cache_read_input_tokens") * pin * 0.1
        + n("output_tokens") * pout)
        / 1_000_000.0;
    // Sem id (raro), usa a própria linha como chave única.
    let id = msg["id"]
        .as_str()
        .map(|m| format!("{m}:{}", v["requestId"].as_str().unwrap_or("")))
        .unwrap_or_else(|| format!("{ts_ms}:{}", v["uuid"].as_str().unwrap_or("")));
    Some(Ev { ts_ms, id, cents: usd * 100.0 })
}

fn collect_files(dir: &Path, min_mtime: SystemTime, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let Ok(md) = e.metadata() else { continue };
        if md.is_dir() {
            collect_files(&p, min_mtime, out);
        } else if p.extension().is_some_and(|x| x == "jsonl")
            && md.modified().map(|m| m >= min_mtime).unwrap_or(true)
        {
            out.push(p);
        }
    }
}

/// Lê só o que foi acrescentado ao arquivo desde a última passada.
fn refresh_file(path: &Path, entry: &mut FileCache) {
    let Ok(f) = File::open(path) else { return };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    if len < entry.offset {
        *entry = FileCache::default(); // arquivo foi recriado
    }
    if len == entry.offset {
        return;
    }
    let mut r = BufReader::new(f);
    if r.seek(SeekFrom::Start(entry.offset)).is_err() {
        return;
    }
    let mut buf = String::new();
    loop {
        buf.clear();
        match r.read_line(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if !buf.ends_with('\n') {
                    break; // linha ainda sendo escrita: tenta de novo na próxima
                }
                entry.offset += n as u64;
                if let Some(ev) = parse_line(&buf) {
                    entry.events.push(ev);
                }
            }
        }
    }
}

fn local_midnight_ms(d: NaiveDate) -> i64 {
    d.and_hms_opt(0, 0, 0)
        .and_then(|t| Local.from_local_datetime(&t).earliest())
        .map(|t| t.timestamp_millis())
        .unwrap_or(0)
}

fn root_dir() -> PathBuf {
    if let Some(p) = config::setting("CLAUDE_DIR") {
        return PathBuf::from(p);
    }
    let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).unwrap_or_default();
    PathBuf::from(home).join(".claude").join("projects")
}

/// Pasta de logs do Claude Code existe nesta máquina?
pub fn available() -> bool {
    root_dir().is_dir()
}

/// Início (ms) do período que interessa: o menor entre o 1º dia do mês e a segunda-feira desta semana.
pub fn period_start_ms() -> i64 {
    let today = Local::now().date_naive();
    let monday = today - Duration::days(today.weekday().num_days_from_monday() as i64);
    local_midnight_ms(today.with_day(1).unwrap().min(monday))
}

/// Mtime mínimo para um arquivo de log ainda ter eventos do período.
pub fn min_mtime() -> SystemTime {
    SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(period_start_ms().max(0) as u64)
}

/// Monta o snapshot (ciclo = mês corrente, ritmo, semana seg–sex) a partir de (instante ms, centavos).
pub fn summarize(events: impl Iterator<Item = (i64, f64)>, settings: &Settings) -> Snapshot {
    let today = Local::now().date_naive();
    let today_idx = today.weekday().num_days_from_monday() as usize;
    let monday = today - Duration::days(today_idx as i64);
    let month_start = today.with_day(1).unwrap();
    let next_month = if month_start.month() == 12 {
        NaiveDate::from_ymd_opt(month_start.year() + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(month_start.year(), month_start.month() + 1, 1)
    }
    .unwrap();
    let end = now_ms() as i64;
    let win_start = end - (settings.window_hours * 3_600_000.0) as i64;
    let month_ms = local_midnight_ms(month_start);
    let day_ms: Vec<i64> = (0..6).map(|i| local_midnight_ms(monday + Duration::days(i))).collect();

    let (mut cycle, mut win, mut win_n) = (0.0, 0.0, 0u64);
    let mut days = [0.0f64; 5];
    for (ts, cents) in events {
        if ts >= month_ms {
            cycle += cents;
        }
        if ts >= win_start {
            win += cents;
            win_n += 1;
        }
        for i in 0..5 {
            if ts >= day_ms[i] && ts < day_ms[i + 1] {
                days[i] += cents;
            }
        }
    }

    let local = |d: NaiveDate| Local.from_local_datetime(&d.and_hms_opt(0, 0, 0).unwrap()).earliest().unwrap().to_rfc3339();
    let mut snap = Snapshot::empty("ok", None, settings);
    snap.cycle_used_usd = cycle / 100.0;
    snap.cycle_start = Some(local(month_start));
    snap.cycle_end = Some(local(next_month));
    snap.window_usd = win / 100.0;
    snap.window_events = win_n;
    snap.rate_per_hour = snap.window_usd / settings.window_hours;
    snap.week_usd = (0..5).map(|i| (i <= today_idx).then_some(days[i] / 100.0)).collect();
    snap.week_today = (today_idx <= 4).then_some(today_idx);
    snap.level = level_of(snap.rate_per_hour, snap.low_max, snap.high_min).into();
    snap
}

fn compute(settings: &Settings) -> Result<Snapshot, String> {
    let root = root_dir();
    if !root.is_dir() {
        return Err(format!("Pasta não encontrada: {}", root.display()));
    }
    let from = period_start_ms();
    let min_mtime = min_mtime();
    let mut files = Vec::new();
    collect_files(&root, min_mtime, &mut files);

    let mut guard = CACHE.lock().unwrap();
    let cache = guard.get_or_insert_with(HashMap::new);
    let mut dedup: HashMap<String, Ev> = HashMap::new();
    for p in &files {
        let entry = cache.entry(p.clone()).or_default();
        refresh_file(p, entry);
        for ev in &entry.events {
            if ev.ts_ms < from {
                continue;
            }
            // Mensagens em streaming repetem o id: vale a linha mais completa (maior custo).
            dedup
                .entry(ev.id.clone())
                .and_modify(|o| {
                    if ev.cents > o.cents {
                        *o = ev.clone();
                    }
                })
                .or_insert_with(|| ev.clone());
        }
    }
    drop(guard);

    let mut snap = summarize(dedup.values().map(|e| (e.ts_ms, e.cents)), settings);
    apply_limits(&mut snap);
    Ok(snap)
}

/// `resets_at` pode vir em segundos, milissegundos ou texto ISO-8601.
pub fn to_ms(v: &Value) -> Option<i64> {
    if let Some(n) = v.as_f64() {
        return Some(if n < 1e11 { (n * 1000.0) as i64 } else { n as i64 });
    }
    DateTime::parse_from_rfc3339(v.as_str()?).ok().map(|t| t.timestamp_millis())
}

/// Lê `~/.claude/widget-limits.json`, gravado pela status line (tools/claude-statusline.ps1).
fn apply_limits(snap: &mut Snapshot) {
    let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).unwrap_or_default();
    let path = PathBuf::from(home).join(".claude").join("widget-limits.json");
    let Ok(text) = fs::read_to_string(path) else { return };
    let Ok(v) = serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}')) else { return };
    let rl = &v["rate_limits"];
    snap.limits_updated_ms = v["updated_at_ms"].as_u64();
    snap.limit_5h_pct = rl["five_hour"]["used_percentage"].as_f64();
    snap.limit_5h_resets_ms = to_ms(&rl["five_hour"]["resets_at"]);
    snap.limit_7d_pct = rl["seven_day"]["used_percentage"].as_f64();
    snap.limit_7d_resets_ms = to_ms(&rl["seven_day"]["resets_at"]);
}

/// Última resposta do endpoint de uso (instante, JSON): consulta no máximo 1x por minuto.
static API: Mutex<Option<(std::time::Instant, Value)>> = Mutex::new(None);

/// Token OAuth do Claude Code (`~/.claude/.credentials.json`); só é enviado para api.anthropic.com.
fn access_token() -> Option<String> {
    let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).ok()?;
    let text = fs::read_to_string(PathBuf::from(home).join(".claude").join(".credentials.json")).ok()?;
    let v: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    let o = &v["claudeAiOauth"];
    if o["expiresAt"].as_i64().is_some_and(|e| e <= now_ms() as i64) {
        return None; // expirado: o Claude Code renova sozinho; até lá vale a status line
    }
    o["accessToken"].as_str().map(str::to_owned)
}

/// Mesmo endpoint do `/usage` do Claude Code (não documentado: falhas caem na status line).
async fn api_usage() -> Option<Value> {
    if let Some((t, v)) = API.lock().unwrap().as_ref() {
        if t.elapsed() < std::time::Duration::from_secs(60) {
            return Some(v.clone());
        }
    }
    let token = access_token()?;
    let v: Value = reqwest::Client::new()
        .get("https://api.anthropic.com/api/oauth/usage")
        .bearer_auth(token)
        .header("anthropic-beta", "oauth-2025-04-20")
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json()
        .await
        .ok()?;
    *API.lock().unwrap() = Some((std::time::Instant::now(), v.clone()));
    Some(v)
}

fn apply_api(snap: &mut Snapshot, v: &Value) {
    let pct = |k: &str| v[k]["utilization"].as_f64();
    if pct("five_hour").is_none() && pct("seven_day").is_none() {
        return;
    }
    snap.limits_updated_ms = Some(now_ms());
    snap.limit_5h_pct = pct("five_hour");
    snap.limit_5h_resets_ms = to_ms(&v["five_hour"]["resets_at"]);
    snap.limit_7d_pct = pct("seven_day");
    snap.limit_7d_resets_ms = to_ms(&v["seven_day"]["resets_at"]);
}

pub async fn fetch(settings: &Settings) -> Result<Snapshot, String> {
    let s = settings.clone();
    let mut snap = tokio::task::spawn_blocking(move || compute(&s)).await.map_err(|e| e.to_string())??;
    if let Some(v) = api_usage().await {
        apply_api(&mut snap, &v);
    }
    Ok(snap)
}
