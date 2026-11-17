//! Consulta dos endpoints internos do dashboard do Cursor (não documentados).
use crate::config;
use reqwest::{Client, Method, StatusCode};
use serde::Serialize;
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};

const BASE: &str = "https://cursor.com";
const WINDOW_HOURS: f64 = 3.0;
const PAGE_SIZE: u64 = 100;
const MAX_PAGES: u64 = 30;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// "loading" | "ok" | "auth" | "error"
    pub status: String,
    pub message: Option<String>,
    pub cycle_used_usd: f64,
    pub cycle_limit_usd: f64,
    pub cycle_start: Option<String>,
    pub cycle_end: Option<String>,
    pub window_usd: f64,
    pub window_events: u64,
    pub rate_per_hour: f64,
    /// "low" | "mid" | "high"
    pub level: String,
    pub low_max: f64,
    pub high_min: f64,
    pub updated_at_ms: u64,
}

impl Snapshot {
    pub fn empty(status: &str, message: Option<String>) -> Self {
        Snapshot {
            status: status.into(),
            message,
            cycle_used_usd: 0.0,
            cycle_limit_usd: config::setting_f64("CYCLE_LIMIT_USD", 1000.0),
            cycle_start: None,
            cycle_end: None,
            window_usd: 0.0,
            window_events: 0,
            rate_per_hour: 0.0,
            level: "low".into(),
            low_max: config::setting_f64("RATE_LOW_MAX", 1.0),
            high_min: config::setting_f64("RATE_HIGH_MIN", 3.0),
            updated_at_ms: now_ms(),
        }
    }
}

pub enum FetchError {
    Auth,
    Other(String),
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

async fn call(
    client: &Client,
    cookie: &str,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> Result<Value, FetchError> {
    let mut req = client
        .request(method, format!("{BASE}{path}"))
        .header("Cookie", format!("WorkosCursorSessionToken={cookie}"))
        .header("Origin", BASE)
        .header("Referer", format!("{BASE}/dashboard"));
    if let Some(b) = body {
        req = req.json(&b);
    }
    let resp = req.send().await.map_err(|e| FetchError::Other(e.to_string()))?;
    match resp.status() {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(FetchError::Auth),
        s if !s.is_success() => Err(FetchError::Other(format!("HTTP {s} em {path}"))),
        _ => resp
            .json::<Value>()
            .await
            .map_err(|e| FetchError::Other(format!("JSON inválido em {path}: {e}"))),
    }
}

/// Soma o valor cobrado (`chargedCents`) dos eventos da janela. Retorna (centavos, nº de eventos).
async fn window_cost(
    client: &Client,
    cookie: &str,
    team_id: i64,
    start: u64,
    end: u64,
) -> Result<(f64, u64), FetchError> {
    let (mut cents, mut seen) = (0.0, 0u64);
    for page in 1..=MAX_PAGES {
        let body = json!({
            "teamId": team_id,
            "startDate": start.to_string(),
            "endDate": end.to_string(),
            "page": page,
            "pageSize": PAGE_SIZE,
        });
        let v = call(client, cookie, Method::POST, "/api/dashboard/get-filtered-usage-events", Some(body)).await?;
        let events = v["usageEventsDisplay"].as_array().cloned().unwrap_or_default();
        if events.is_empty() {
            break;
        }
        for e in &events {
            cents += e["chargedCents"]
                .as_f64()
                .or_else(|| e["tokenUsage"]["totalCents"].as_f64())
                .unwrap_or(0.0);
        }
        seen += events.len() as u64;
        if seen >= v["totalUsageEventsCount"].as_u64().unwrap_or(0) {
            break;
        }
    }
    Ok((cents, seen))
}

pub async fn fetch(
    client: &Client,
    cookie: &str,
    team_cache: &mut Option<i64>,
) -> Result<Snapshot, FetchError> {
    let summary = call(client, cookie, Method::GET, "/api/usage-summary", None).await?;

    if team_cache.is_none() {
        if let Ok(t) = call(client, cookie, Method::POST, "/api/dashboard/teams", Some(json!({}))).await {
            *team_cache = t["teams"][0]["id"].as_i64();
        }
    }
    let team_id = team_cache.unwrap_or(-1);

    let end = now_ms();
    let start = end.saturating_sub((WINDOW_HOURS * 3_600_000.0) as u64);
    let (cents, events) = window_cost(client, cookie, team_id, start, end).await?;

    let overall = &summary["individualUsage"]["overall"];
    let mut snap = Snapshot::empty("ok", None);
    snap.cycle_used_usd = overall["used"].as_f64().unwrap_or(0.0) / 100.0;
    if let Some(limit) = overall["limit"].as_f64() {
        snap.cycle_limit_usd = limit / 100.0;
    }
    snap.cycle_start = summary["billingCycleStart"].as_str().map(String::from);
    snap.cycle_end = summary["billingCycleEnd"].as_str().map(String::from);
    snap.window_usd = cents / 100.0;
    snap.window_events = events;
    snap.rate_per_hour = snap.window_usd / WINDOW_HOURS;
    snap.level = if snap.rate_per_hour < snap.low_max {
        "low"
    } else if snap.rate_per_hour < snap.high_min {
        "mid"
    } else {
        "high"
    }
    .into();
    Ok(snap)
}
