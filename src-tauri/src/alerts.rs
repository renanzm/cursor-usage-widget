//! Notificações do Windows: cruzamento de 75/90/100% do ciclo e ritmo "Alto".
use crate::{i18n::is_en, usage::Snapshot};
use std::time::{Duration, Instant};
use tauri_plugin_notification::NotificationExt;

const HIGH_COOLDOWN: Duration = Duration::from_secs(30 * 60);

#[derive(Default)]
pub struct Alerts {
    initialized: bool,
    cycle_start: Option<String>,
    stage: u8,
    was_high: bool,
    last_high_alert: Option<Instant>,
}

fn stage_of(pct: f64) -> u8 {
    if pct >= 100.0 {
        100
    } else if pct >= 90.0 {
        90
    } else if pct >= 75.0 {
        75
    } else {
        0
    }
}

fn usd(v: f64) -> String {
    if is_en() {
        format!("${v:.2}")
    } else {
        format!("US$ {v:.2}").replace('.', ",")
    }
}

fn hours(h: f64) -> String {
    if h.fract() == 0.0 {
        format!("{h:.0}")
    } else if is_en() {
        format!("{h:.1}")
    } else {
        format!("{h:.1}").replace('.', ",")
    }
}

pub fn notify(app: &tauri::AppHandle, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
}

impl Alerts {
    pub fn check(&mut self, app: &tauri::AppHandle, s: &Snapshot) {
        if s.status != "ok" {
            return;
        }
        let pct = if s.cycle_limit_usd > 0.0 {
            s.cycle_used_usd / s.cycle_limit_usd * 100.0
        } else {
            0.0
        };
        let stage = stage_of(pct);
        let high = s.level == "high";
        let src = match s.source.as_str() {
            "claude" => "Claude Code",
            "codex" => "Codex",
            _ => "Cursor",
        };

        // Novo ciclo mensal ou primeira leitura: apenas registra o estado, sem avisar.
        if !self.initialized || self.cycle_start != s.cycle_start {
            self.initialized = true;
            self.cycle_start = s.cycle_start.clone();
            self.stage = stage;
            self.was_high = high;
            return;
        }

        if stage > self.stage {
            let restante = (s.cycle_limit_usd - s.cycle_used_usd).max(0.0);
            let (used, limit, left) = (usd(s.cycle_used_usd), usd(s.cycle_limit_usd), usd(restante));
            let (title, body) = match (stage, is_en()) {
                (100, false) => (format!("{src}: limite do ciclo atingido"), format!("{used} de {limit} usados.")),
                (100, true) => (format!("{src}: cycle limit reached"), format!("{used} of {limit} used.")),
                (n, false) => (
                    format!("{src}: {n}% do limite do ciclo"),
                    format!("{used} de {limit} usados · restam {left}."),
                ),
                (n, true) => (format!("{src}: {n}% of the cycle limit"), format!("{used} of {limit} used · {left} left.")),
            };
            notify(app, &title, &body);
        }
        self.stage = stage;

        if high && !self.was_high {
            let cooled = self
                .last_high_alert
                .map(|t| t.elapsed() >= HIGH_COOLDOWN)
                .unwrap_or(true);
            if cooled {
                let (rate, h, win) = (usd(s.rate_per_hour), hours(s.window_hours), usd(s.window_usd));
                let (title, body) = if is_en() {
                    (format!("{src}: high spending pace"), format!("{rate}/h over the last {h}h ({win} in the period)."))
                } else {
                    (format!("{src}: ritmo de gasto alto"), format!("{rate}/h nas últimas {h}h ({win} no período)."))
                };
                notify(app, &title, &body);
                self.last_high_alert = Some(Instant::now());
            }
        }
        self.was_high = high;
    }
}
