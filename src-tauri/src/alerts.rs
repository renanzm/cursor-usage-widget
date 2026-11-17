//! Notificações do Windows: cruzamento de 75/90/100% do ciclo e ritmo "Alto".
use crate::usage::Snapshot;
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
    format!("US$ {v:.2}")
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
            let (title, body) = match stage {
                100 => (
                    "Cursor: limite do ciclo atingido".to_string(),
                    format!("{} de {} usados.", usd(s.cycle_used_usd), usd(s.cycle_limit_usd)),
                ),
                n => (
                    format!("Cursor: {n}% do limite do ciclo"),
                    format!(
                        "{} de {} usados · restam {}.",
                        usd(s.cycle_used_usd),
                        usd(s.cycle_limit_usd),
                        usd(restante)
                    ),
                ),
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
                notify(
                    app,
                    "Cursor: ritmo de gasto alto",
                    &format!(
                        "{}/h nas últimas 3h ({} no período).",
                        usd(s.rate_per_hour),
                        usd(s.window_usd)
                    ),
                );
                self.last_high_alert = Some(Instant::now());
            }
        }
        self.was_high = high;
    }
}
