//! Configurações do usuário (painel): `settings.json` na pasta de dados do app.
use crate::{config, i18n::tr};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

const IDENTIFIER: &str = "dev.local.cursor-usage-widget";

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum QuotaMode {
    /// Limite informado pelo Cursor; `quota_usd` só se o Cursor não informar.
    #[default]
    Auto,
    Manual,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

/// De onde ler o uso. `Auto` usa a primeira disponível: Claude Code, Codex, Cursor.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    #[default]
    Auto,
    Claude,
    Codex,
    Cursor,
}

impl Source {
    fn from_str(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "claude" => Source::Claude,
            "codex" => Source::Codex,
            "cursor" => Source::Cursor,
            _ => Source::Auto,
        }
    }
}

/// Idioma da interface. `Auto` segue o Windows.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    Auto,
    En,
    Pt,
}

/// Borda da tela em que o widget fica encostado.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Left,
    #[default]
    Right,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Colors {
    pub accent: String,
    pub low: String,
    pub mid: String,
    pub high: String,
}

impl Default for Colors {
    fn default() -> Self {
        Colors {
            accent: "#ff9f0a".into(),
            low: "#30d158".into(),
            mid: "#ffd60a".into(),
            high: "#ff453a".into(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// Fonte dos dados (Claude Code, Codex ou Cursor).
    #[serde(default)]
    pub source: Source,
    pub quota_mode: QuotaMode,
    pub quota_usd: f64,
    pub window_hours: f64,
    /// Fundo de escala do velocímetro (US$/h).
    pub rate_max: f64,
    pub low_max: f64,
    pub high_min: f64,
    /// Fundo de escala diário do equalizador (US$).
    pub day_max_usd: f64,
    pub poll_seconds: f64,
    /// Tema visual do widget e do painel.
    #[serde(default)]
    pub theme: Theme,
    /// Nome do monitor (ex.: `\\.\DISPLAY2`); vazio = o monitor mais à direita.
    #[serde(default)]
    pub monitor: String,
    /// Lado do monitor em que o widget fica.
    #[serde(default)]
    pub side: Side,
    /// Idioma dos textos.
    #[serde(default)]
    pub language: Language,
    pub colors: Colors,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            source: Source::Auto,
            quota_mode: QuotaMode::Auto,
            quota_usd: 1000.0,
            window_hours: 3.0,
            rate_max: 20.0,
            low_max: 6.0,
            high_min: 13.0,
            day_max_usd: 100.0,
            poll_seconds: 30.0,
            theme: Theme::Dark,
            monitor: String::new(),
            side: Side::Right,
            language: Language::Auto,
            colors: Colors::default(),
        }
    }
}

#[derive(Serialize, Debug)]
pub struct FieldError {
    pub field: &'static str,
    pub message: String,
}

#[derive(Serialize, Debug)]
pub struct SaveError {
    pub message: String,
    pub fields: Vec<FieldError>,
}

fn is_hex_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

impl Settings {
    pub fn validate(&self) -> Result<(), Vec<FieldError>> {
        let mut errs = Vec::new();
        let mut err = |field: &'static str, pt: &str, en: &str| {
            errs.push(FieldError { field, message: tr(pt, en).into() })
        };
        if !(self.quota_usd.is_finite() && self.quota_usd > 0.0) {
            err("quotaUsd", "A cota precisa ser maior que zero.", "The quota must be greater than zero.");
        }
        if !(self.window_hours.is_finite() && self.window_hours >= 0.5 && self.window_hours <= 24.0) {
            err("windowHours", "A janela precisa estar entre 0,5 e 24 horas.", "The window must be between 0.5 and 24 hours.");
        }
        if !(self.low_max.is_finite() && self.low_max > 0.0) {
            err("lowMax", "Baixo precisa ser maior que zero.", "Low must be greater than zero.");
        } else if !(self.low_max < self.high_min) {
            err("lowMax", "Baixo precisa ser menor que Alto.", "Low must be less than High.");
        }
        if !(self.high_min.is_finite() && self.rate_max.is_finite() && self.high_min < self.rate_max) {
            err("highMin", "Alto precisa ser menor que o fim da escala.", "High must be less than the end of the scale.");
        }
        if !(self.day_max_usd.is_finite() && self.day_max_usd >= 1.0) {
            err("dayMaxUsd", "A escala diária precisa ser de pelo menos 1.", "The daily scale must be at least 1.");
        }
        if !(self.poll_seconds.is_finite() && self.poll_seconds >= 10.0 && self.poll_seconds <= 3600.0) {
            err("pollSeconds", "O intervalo precisa estar entre 10 e 3600 segundos.", "The interval must be between 10 and 3600 seconds.");
        }
        for (field, value) in [
            ("colors.accent", &self.colors.accent),
            ("colors.low", &self.colors.low),
            ("colors.mid", &self.colors.mid),
            ("colors.high", &self.colors.high),
        ] {
            if !is_hex_color(value) {
                err(field, "Cor inválida (use #rrggbb).", "Invalid color (use #rrggbb).");
            }
        }
        if errs.is_empty() {
            Ok(())
        } else {
            Err(errs)
        }
    }

    /// Troca pelo padrão os campos que violam alguma regra. Limites do ritmo voltam juntos.
    pub fn sanitize(mut self) -> Self {
        let Err(errs) = self.validate() else { return self };
        let d = Settings::default();
        for e in errs {
            match e.field {
                "quotaUsd" => self.quota_usd = d.quota_usd,
                "windowHours" => self.window_hours = d.window_hours,
                "lowMax" | "highMin" => {
                    self.low_max = d.low_max;
                    self.high_min = d.high_min;
                    self.rate_max = d.rate_max;
                }
                "dayMaxUsd" => self.day_max_usd = d.day_max_usd,
                "pollSeconds" => self.poll_seconds = d.poll_seconds,
                "colors.accent" => self.colors.accent = d.colors.accent.clone(),
                "colors.low" => self.colors.low = d.colors.low.clone(),
                "colors.mid" => self.colors.mid = d.colors.mid.clone(),
                "colors.high" => self.colors.high = d.colors.high.clone(),
                _ => {}
            }
        }
        self
    }

    /// Cota efetiva do ciclo (US$).
    pub fn cycle_limit(&self, cursor_limit_usd: Option<f64>) -> f64 {
        match self.quota_mode {
            QuotaMode::Manual => self.quota_usd,
            QuotaMode::Auto => cursor_limit_usd.unwrap_or(self.quota_usd),
        }
    }
}

/// Lê o JSON campo a campo: o que faltar, tiver tipo errado ou violar regra vira padrão.
pub fn parse(text: &str) -> Settings {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let Ok(v) = serde_json::from_str::<Value>(text) else {
        return Settings::default();
    };
    let d = Settings::default();
    let num = |k: &str, def: f64| v[k].as_f64().unwrap_or(def);
    let color = |k: &str, def: &str| v["colors"][k].as_str().unwrap_or(def).to_string();
    let quota_mode = match v["quotaMode"].as_str() {
        Some("manual") => QuotaMode::Manual,
        _ => QuotaMode::Auto,
    };
    let theme = match v["theme"].as_str() {
        Some("light") => Theme::Light,
        _ => Theme::Dark,
    };
    Settings {
        source: Source::from_str(v["source"].as_str().unwrap_or("")),
        quota_mode,
        theme,
        monitor: v["monitor"].as_str().unwrap_or("").to_string(),
        side: if v["side"].as_str() == Some("left") { Side::Left } else { Side::Right },
        language: match v["language"].as_str() {
            Some("en") => Language::En,
            Some("pt") => Language::Pt,
            _ => Language::Auto,
        },
        quota_usd: num("quotaUsd", d.quota_usd),
        window_hours: num("windowHours", d.window_hours),
        rate_max: num("rateMax", d.rate_max),
        low_max: num("lowMax", d.low_max),
        high_min: num("highMin", d.high_min),
        day_max_usd: num("dayMaxUsd", d.day_max_usd),
        poll_seconds: num("pollSeconds", d.poll_seconds),
        colors: Colors {
            accent: color("accent", &d.colors.accent),
            low: color("low", &d.colors.low),
            mid: color("mid", &d.colors.mid),
            high: color("high", &d.colors.high),
        },
    }
    .sanitize()
}

/// Valores iniciais a partir do `.env`/variáveis de ambiente (instalações anteriores ao painel).
pub fn from_env() -> Settings {
    let d = Settings::default();
    Settings {
        source: Source::from_str(&config::setting("USAGE_SOURCE").unwrap_or_default()),
        quota_usd: config::setting_f64("CYCLE_LIMIT_USD", d.quota_usd),
        rate_max: config::setting_f64("RATE_MAX", d.rate_max),
        low_max: config::setting_f64("RATE_LOW_MAX", d.low_max),
        high_min: config::setting_f64("RATE_HIGH_MIN", d.high_min),
        day_max_usd: config::setting_f64("DAY_MAX_USD", d.day_max_usd),
        poll_seconds: config::setting_f64("POLL_SECONDS", d.poll_seconds),
        ..d
    }
    .sanitize()
}

/// `%APPDATA%\<identificador>\settings.json`, a mesma pasta de `app_config_dir()` do Tauri.
pub fn default_path() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join(IDENTIFIER).join("settings.json"))
}

pub fn load_or_init(path: Option<&Path>) -> Settings {
    let Some(path) = path else { return from_env() };
    match fs::read_to_string(path) {
        Ok(text) => parse(&text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let s = from_env();
            let _ = save(path, &s);
            s
        }
        Err(_) => Settings::default(),
    }
}

pub fn save(path: &Path, s: &Settings) -> Result<(), String> {
    let fail = |e: std::io::Error| format!("{}: {e}", tr("Falha ao gravar as configurações", "Could not save the settings"));
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(fail)?;
    }
    let text = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
    fs::write(path, text).map_err(fail)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(s: &Settings) -> Vec<&'static str> {
        s.validate()
            .err()
            .unwrap_or_default()
            .into_iter()
            .map(|e| e.field)
            .collect()
    }

    fn temp_path(tag: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!("cuw-{tag}-{}", std::process::id()))
            .join("settings.json")
    }

    #[test]
    fn default_is_valid() {
        assert!(Settings::default().validate().is_ok());
    }

    #[test]
    fn window_hours_bounds() {
        for (h, ok) in [(0.5, true), (24.0, true), (0.4, false), (24.5, false)] {
            let s = Settings { window_hours: h, ..Default::default() };
            assert_eq!(fields(&s).contains(&"windowHours"), !ok, "janela {h}");
        }
    }

    #[test]
    fn low_must_be_positive_and_below_high() {
        let s = Settings { low_max: 13.0, high_min: 13.0, ..Default::default() };
        assert_eq!(fields(&s), vec!["lowMax"]);
        let s = Settings { low_max: 0.0, ..Default::default() };
        assert_eq!(fields(&s), vec!["lowMax"]);
    }

    #[test]
    fn high_must_be_below_rate_max() {
        let s = Settings { high_min: 20.0, rate_max: 20.0, ..Default::default() };
        assert_eq!(fields(&s), vec!["highMin"]);
    }

    #[test]
    fn other_minimums() {
        let s = Settings { quota_usd: 0.0, day_max_usd: 0.5, poll_seconds: 9.0, ..Default::default() };
        assert_eq!(fields(&s), vec!["quotaUsd", "dayMaxUsd", "pollSeconds"]);
    }

    #[test]
    fn poll_seconds_bounds() {
        for (p, ok) in [(10.0, true), (3600.0, true), (9.0, false), (3601.0, false), (1e20, false), (f64::INFINITY, false)] {
            let s = Settings { poll_seconds: p, ..Default::default() };
            assert_eq!(fields(&s).contains(&"pollSeconds"), !ok, "intervalo {p}");
        }
    }

    #[test]
    fn non_finite_numbers_are_invalid() {
        let s = Settings { quota_usd: f64::INFINITY, ..Default::default() };
        assert_eq!(fields(&s), vec!["quotaUsd"]);
        let s = Settings { window_hours: f64::NAN, ..Default::default() };
        assert_eq!(fields(&s), vec!["windowHours"]);
        let s = Settings { rate_max: f64::INFINITY, ..Default::default() };
        assert!(fields(&s).contains(&"highMin"));
    }

    #[test]
    fn sanitize_replaces_huge_poll_seconds() {
        assert_eq!(Settings { poll_seconds: 1e20, ..Default::default() }.sanitize().poll_seconds, 30.0);
    }

    #[test]
    fn colors_must_be_hex() {
        let mut s = Settings::default();
        s.colors.accent = "red".into();
        s.colors.high = "#12345g".into();
        assert_eq!(fields(&s), vec!["colors.accent", "colors.high"]);
    }

    #[test]
    fn parse_incomplete_keeps_known_fields() {
        let s = parse(r##"{"quotaMode":"manual","quotaUsd":250,"colors":{"accent":"#0a84ff"}}"##);
        assert_eq!(s.quota_mode, QuotaMode::Manual);
        assert_eq!(s.quota_usd, 250.0);
        assert_eq!(s.colors.accent, "#0a84ff");
        assert_eq!(s.window_hours, 3.0);
        assert_eq!(s.colors.low, "#30d158");
    }

    #[test]
    fn parse_invalid_values_fall_back_per_field() {
        let s = parse(r#"{"windowHours":"x","pollSeconds":2,"dayMaxUsd":50,"lowMax":15}"#);
        assert_eq!(s.window_hours, 3.0);
        assert_eq!(s.poll_seconds, 30.0);
        assert_eq!(s.day_max_usd, 50.0);
        assert_eq!((s.low_max, s.high_min, s.rate_max), (6.0, 13.0, 20.0));
    }

    #[test]
    fn parse_accepts_utf8_bom() {
        let s = parse("\u{feff}{\"quotaMode\":\"manual\",\"quotaUsd\":250}");
        assert_eq!(s.quota_mode, QuotaMode::Manual);
        assert_eq!(s.quota_usd, 250.0);
    }

    #[test]
    fn parse_theme() {
        assert_eq!(parse(r#"{"theme":"light"}"#).theme, Theme::Light);
        assert_eq!(parse(r#"{"theme":"sepia"}"#).theme, Theme::Dark);
        assert_eq!(parse("{}").theme, Theme::Dark);
    }

    #[test]
    fn theme_roundtrips_as_lowercase() {
        let s = Settings { theme: Theme::Light, ..Default::default() };
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["theme"], "light");
        assert_eq!(serde_json::from_value::<Settings>(json).unwrap().theme, Theme::Light);
    }

    #[test]
    fn parse_source() {
        assert_eq!(parse(r#"{"source":"codex"}"#).source, Source::Codex);
        assert_eq!(parse(r#"{"source":"Cursor"}"#).source, Source::Cursor);
        assert_eq!(parse(r#"{"source":"x"}"#).source, Source::Auto);
        assert_eq!(parse("{}").source, Source::Auto);
        let json = serde_json::to_value(Settings { source: Source::Claude, ..Default::default() }).unwrap();
        assert_eq!(json["source"], "claude");
    }

    #[test]
    fn parse_language() {
        assert_eq!(parse(r#"{"language":"en"}"#).language, Language::En);
        assert_eq!(parse(r#"{"language":"pt"}"#).language, Language::Pt);
        assert_eq!(parse(r#"{"language":"fr"}"#).language, Language::Auto);
        let json = serde_json::to_value(Settings { language: Language::Pt, ..Default::default() }).unwrap();
        assert_eq!(json["language"], "pt");
    }

    #[test]
    fn parse_position() {
        let s = parse(r#"{"monitor":"\\\\.\\DISPLAY2","side":"left"}"#);
        assert_eq!((s.monitor.as_str(), s.side), (r"\\.\DISPLAY2", Side::Left));
        let s = parse(r#"{"side":"top"}"#);
        assert_eq!((s.monitor.as_str(), s.side), ("", Side::Right));
        let json = serde_json::to_value(Settings { side: Side::Left, ..Default::default() }).unwrap();
        assert_eq!(json["side"], "left");
    }

    #[test]
    fn parse_corrupted_json_gives_defaults() {
        assert_eq!(parse("{ nao é json"), Settings::default());
    }

    #[test]
    fn cycle_limit_by_mode() {
        let auto = Settings { quota_usd: 500.0, ..Default::default() };
        assert_eq!(auto.cycle_limit(Some(2000.0)), 2000.0);
        assert_eq!(auto.cycle_limit(None), 500.0);
        let manual = Settings { quota_mode: QuotaMode::Manual, quota_usd: 500.0, ..Default::default() };
        assert_eq!(manual.cycle_limit(Some(2000.0)), 500.0);
    }

    #[test]
    fn save_then_load_roundtrip() {
        let path = temp_path("roundtrip");
        let s = Settings { quota_mode: QuotaMode::Manual, quota_usd: 321.0, ..Default::default() };
        save(&path, &s).unwrap();
        assert_eq!(load_or_init(Some(&path)), s);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn load_or_init_creates_missing_file() {
        let path = temp_path("init");
        let _ = fs::remove_dir_all(path.parent().unwrap());
        let s = load_or_init(Some(&path));
        assert!(s.validate().is_ok());
        assert!(path.exists());
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }
}
