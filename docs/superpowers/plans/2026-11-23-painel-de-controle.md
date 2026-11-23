# Painel de controle (configurações) — plano de implementação

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Janela de Configurações que permite ajustar cota mensal, janela e escala do ritmo, limites baixo/médio/alto, escala do equalizador, frequência de atualização e cores, com efeito imediato no widget.

**Architecture:** Um módulo Rust `settings.rs` é dono das configurações (padrões, validação, leitura tolerante e gravação em `%APPDATA%\dev.local.cursor-usage-widget\settings.json`). `usage.rs`, `alerts.rs` e `main.rs` passam a ler dele em vez do `.env`. Comandos Tauri expõem leitura/gravação para uma nova janela `settings.html`; ao salvar, o Rust emite o evento `settings` (o widget aplica as cores via variáveis CSS) e força uma nova consulta.

**Tech Stack:** Rust 2021, Tauri 2, serde/serde_json, tokio; HTML/CSS/JS puro no WebView2 (sem bundler).

**Spec:** `docs/superpowers/specs/2026-11-23-painel-de-controle-design.md`

## Global Constraints

- Plataforma: somente Windows (WebView2). CSS pode usar `color-mix()` e sintaxe de cor relativa (`oklch(from …)`).
- Nenhuma dependência nova no `Cargo.toml`.
- Textos da interface e mensagens de erro em português do Brasil.
- Arquivo de configurações: `%APPDATA%\dev.local.cursor-usage-widget\settings.json` (o identificador vem de `tauri.conf.json`).
- Chaves JSON em camelCase: `quotaMode`, `quotaUsd`, `windowHours`, `rateMax`, `lowMax`, `highMin`, `dayMaxUsd`, `pollSeconds`, `colors.{accent,low,mid,high}`.
- Padrões: `quotaMode` `"auto"`, `quotaUsd` 1000, `windowHours` 3, `rateMax` 20, `lowMax` 6, `highMin` 13, `dayMaxUsd` 100, `pollSeconds` 30, cores `#ff9f0a` / `#30d158` / `#ffd60a` / `#ff453a`.
- Regras: `quotaUsd > 0`; `0,5 ≤ windowHours ≤ 24`; `0 < lowMax < highMin`; `highMin < rateMax`; `dayMaxUsd ≥ 1`; `pollSeconds ≥ 10`; cores `#rrggbb`.
- O cookie continua no Credential Manager; nada de cookie no `settings.json`.
- Comandos `cargo` rodam em `src-tauri/`.

## Antes de começar

`ui/style.css` tem uma alteração local não commitada (o dia acima da escala no equalizador pisca em vermelho só no segmento de baixo). A Tarefa 4 edita esse arquivo. Com o aval do usuário, commitar essa alteração isolada primeiro:

```powershell
git add ui/style.css
git commit -m "fix(ui): dia acima da escala pisca em vermelho so no primeiro segmento"
```

Se houver uma instância instalada do widget rodando (início com o Windows), feche-a pela bandeja antes dos testes manuais para não confundir os dois ícones.

## Estrutura de arquivos

| Arquivo | Ação | Responsabilidade |
|---|---|---|
| `src-tauri/src/settings.rs` | Criar | Tipos, padrões, validação, sanitização, leitura/gravação, cota efetiva |
| `src-tauri/src/usage.rs` | Modificar | Usar `Settings` (janela, limites, cota); `level_of`; `windowHours` no snapshot |
| `src-tauri/src/alerts.rs` | Modificar | Texto da notificação com a janela configurada |
| `src-tauri/src/main.rs` | Modificar | Estado com configurações, intervalo dinâmico, comandos, janela e item da bandeja |
| `src-tauri/capabilities/default.json` | Modificar | Liberar a janela `settings` e o fechamento de janela |
| `ui/settings.html`, `ui/settings.js`, `ui/settings.css` | Criar | Formulário de configurações |
| `ui/index.html`, `ui/app.js`, `ui/style.css` | Modificar | Cores por variáveis, rótulo da janela, engrenagem |

---

### Task 1: Módulo `settings.rs`

**Files:**
- Create: `src-tauri/src/settings.rs`
- Modify: `src-tauri/src/main.rs:3-5` (declaração de módulos)
- Test: `src-tauri/src/settings.rs` (módulo `tests`)

**Interfaces:**
- Consumes: `crate::config::setting_f64(key: &str, default: f64) -> f64` (já existe).
- Produces:
  - `pub enum QuotaMode { Auto, Manual }` (serde `"auto"`/`"manual"`, `Default = Auto`)
  - `pub struct Colors { pub accent: String, pub low: String, pub mid: String, pub high: String }`
  - `pub struct Settings { pub quota_mode: QuotaMode, pub quota_usd: f64, pub window_hours: f64, pub rate_max: f64, pub low_max: f64, pub high_min: f64, pub day_max_usd: f64, pub poll_seconds: f64, pub colors: Colors }` (`Serialize + Deserialize + Clone + Default + PartialEq`, camelCase)
  - `pub struct FieldError { pub field: &'static str, pub message: String }` (`Serialize`)
  - `pub struct SaveError { pub message: String, pub fields: Vec<FieldError> }` (`Serialize`)
  - `Settings::validate(&self) -> Result<(), Vec<FieldError>>`
  - `Settings::sanitize(self) -> Settings`
  - `Settings::cycle_limit(&self, cursor_limit_usd: Option<f64>) -> f64`
  - `pub fn parse(text: &str) -> Settings`
  - `pub fn from_env() -> Settings`
  - `pub fn default_path() -> Option<PathBuf>`
  - `pub fn load_or_init(path: Option<&Path>) -> Settings`
  - `pub fn save(path: &Path, s: &Settings) -> Result<(), String>`

- [ ] **Step 1: Registrar o módulo**

Em `src-tauri/src/main.rs`, trocar:

```rust
mod alerts;
mod config;
mod usage;
```

por:

```rust
mod alerts;
mod config;
mod settings;
mod usage;
```

- [ ] **Step 2: Escrever os testes (arquivo ainda sem implementação)**

Criar `src-tauri/src/settings.rs` só com os testes:

```rust
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
```

- [ ] **Step 3: Rodar os testes e ver falhar**

Run: `cargo test settings::`
Expected: erro de compilação (`cannot find type Settings in this scope` e similares).

- [ ] **Step 4: Implementar o módulo**

Inserir no topo de `src-tauri/src/settings.rs`, antes do `#[cfg(test)]`:

```rust
//! Configurações do usuário (painel): `settings.json` na pasta de dados do app.
use crate::config;
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
    pub colors: Colors,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            quota_mode: QuotaMode::Auto,
            quota_usd: 1000.0,
            window_hours: 3.0,
            rate_max: 20.0,
            low_max: 6.0,
            high_min: 13.0,
            day_max_usd: 100.0,
            poll_seconds: 30.0,
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
        let mut err = |field: &'static str, message: &str| {
            errs.push(FieldError { field, message: message.into() })
        };
        if !(self.quota_usd > 0.0) {
            err("quotaUsd", "A cota precisa ser maior que zero.");
        }
        if !(self.window_hours >= 0.5 && self.window_hours <= 24.0) {
            err("windowHours", "A janela precisa estar entre 0,5 e 24 horas.");
        }
        if !(self.low_max > 0.0) {
            err("lowMax", "Baixo precisa ser maior que zero.");
        } else if !(self.low_max < self.high_min) {
            err("lowMax", "Baixo precisa ser menor que Alto.");
        }
        if !(self.high_min < self.rate_max) {
            err("highMin", "Alto precisa ser menor que o fim da escala.");
        }
        if !(self.day_max_usd >= 1.0) {
            err("dayMaxUsd", "A escala diária precisa ser de pelo menos 1.");
        }
        if !(self.poll_seconds >= 10.0) {
            err("pollSeconds", "O intervalo mínimo é de 10 segundos.");
        }
        for (field, value) in [
            ("colors.accent", &self.colors.accent),
            ("colors.low", &self.colors.low),
            ("colors.mid", &self.colors.mid),
            ("colors.high", &self.colors.high),
        ] {
            if !is_hex_color(value) {
                err(field, "Cor inválida (use #rrggbb).");
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
    Settings {
        quota_mode,
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
    let fail = |e: std::io::Error| format!("Falha ao gravar as configurações: {e}");
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(fail)?;
    }
    let text = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
    fs::write(path, text).map_err(fail)
}
```

- [ ] **Step 5: Rodar os testes e ver passar**

Run: `cargo test settings::`
Expected: `test result: ok. 12 passed` (avisos de "never used" são esperados até a Tarefa 2).

- [ ] **Step 6: Commit**

```powershell
git add src-tauri/src/settings.rs src-tauri/src/main.rs
git commit -m "feat(settings): modulo de configuracoes com validacao e leitura tolerante"
```

---

### Task 2: Backend usa as configurações

**Files:**
- Modify: `src-tauri/src/usage.rs` (imports, `WINDOW_HOURS`, `Snapshot`, `Snapshot::empty`, `fetch`, novo `level_of` + testes)
- Modify: `src-tauri/src/alerts.rs` (texto da notificação de ritmo alto)
- Modify: `src-tauri/src/main.rs` (`AppState`, `run_once`, `poll_loop`, `main`)

**Interfaces:**
- Consumes: `Settings`, `Settings::cycle_limit`, `settings::default_path`, `settings::load_or_init` (Tarefa 1).
- Produces:
  - `pub fn level_of(rate: f64, low_max: f64, high_min: f64) -> &'static str` em `usage.rs`
  - `Snapshot::empty(status: &str, message: Option<String>, s: &Settings) -> Snapshot`
  - campo `Snapshot.window_hours: f64` (JSON `windowHours`)
  - `usage::fetch(client, cookie, settings: &Settings, team_cache, day_cache)`
  - `AppState { snapshot, settings: Mutex<Settings>, settings_path: Option<PathBuf>, team, days, notify }` e `AppState::new() -> AppState`

- [ ] **Step 1: Escrever o teste de `level_of`**

No fim de `src-tauri/src/usage.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::level_of;

    #[test]
    fn level_boundaries_with_custom_limits() {
        assert_eq!(level_of(2.0, 2.0, 5.0), "low");
        assert_eq!(level_of(2.01, 2.0, 5.0), "mid");
        assert_eq!(level_of(4.99, 2.0, 5.0), "mid");
        assert_eq!(level_of(5.0, 2.0, 5.0), "high");
    }
}
```

- [ ] **Step 2: Rodar e ver falhar**

Run: `cargo test usage::`
Expected: erro de compilação `cannot find function level_of`.

- [ ] **Step 3: Alterar `usage.rs`**

Trocar a linha `use crate::config;` por:

```rust
use crate::settings::Settings;
```

Remover a constante:

```rust
const WINDOW_HOURS: f64 = 3.0;
```

No `struct Snapshot`, logo depois de `pub rate_per_hour: f64,`, adicionar:

```rust
    /// Janela (horas) usada no cálculo do ritmo.
    pub window_hours: f64,
```

Substituir o `impl Snapshot { ... }` inteiro por:

```rust
impl Snapshot {
    pub fn empty(status: &str, message: Option<String>, s: &Settings) -> Self {
        Snapshot {
            status: status.into(),
            message,
            cycle_used_usd: 0.0,
            cycle_limit_usd: s.cycle_limit(None),
            cycle_start: None,
            cycle_end: None,
            window_usd: 0.0,
            window_events: 0,
            rate_per_hour: 0.0,
            window_hours: s.window_hours,
            level: "low".into(),
            low_max: s.low_max,
            high_min: s.high_min,
            rate_max: s.rate_max,
            week_usd: Vec::new(),
            week_today: None,
            day_max_usd: s.day_max_usd,
            updated_at_ms: now_ms(),
        }
    }
}

/// Baixo: até `low_max` (inclusive); Médio: abaixo de `high_min`; Alto: `high_min` ou mais.
pub fn level_of(rate: f64, low_max: f64, high_min: f64) -> &'static str {
    if rate <= low_max {
        "low"
    } else if rate < high_min {
        "mid"
    } else {
        "high"
    }
}
```

Substituir a função `fetch` inteira por:

```rust
pub async fn fetch(
    client: &Client,
    cookie: &str,
    settings: &Settings,
    team_cache: &mut Option<i64>,
    day_cache: &mut HashMap<String, f64>,
) -> Result<Snapshot, FetchError> {
    let summary = call(client, cookie, Method::GET, "/api/usage-summary", None).await?;

    if team_cache.is_none() {
        if let Ok(t) = call(client, cookie, Method::POST, "/api/dashboard/teams", Some(json!({}))).await {
            *team_cache = t["teams"][0]["id"].as_i64();
        }
    }
    let team_id = team_cache.unwrap_or(-1);

    let end = now_ms();
    let start = end.saturating_sub((settings.window_hours * 3_600_000.0) as u64);
    let (cents, events) = window_cost(client, cookie, team_id, start, end).await?;

    let (week, week_today) = week_usage(client, cookie, team_id, day_cache).await?;

    let overall = &summary["individualUsage"]["overall"];
    let mut snap = Snapshot::empty("ok", None, settings);
    snap.week_usd = week;
    snap.week_today = week_today;
    snap.cycle_used_usd = overall["used"].as_f64().unwrap_or(0.0) / 100.0;
    snap.cycle_limit_usd = settings.cycle_limit(overall["limit"].as_f64().map(|c| c / 100.0));
    snap.cycle_start = summary["billingCycleStart"].as_str().map(String::from);
    snap.cycle_end = summary["billingCycleEnd"].as_str().map(String::from);
    snap.window_usd = cents / 100.0;
    snap.window_events = events;
    snap.rate_per_hour = snap.window_usd / settings.window_hours;
    snap.level = level_of(snap.rate_per_hour, snap.low_max, snap.high_min).into();
    Ok(snap)
}
```

- [ ] **Step 4: Alterar `alerts.rs`**

Depois da função `usd`, adicionar:

```rust
fn hours(h: f64) -> String {
    if h.fract() == 0.0 {
        format!("{h:.0}")
    } else {
        format!("{h:.1}").replace('.', ",")
    }
}
```

No bloco da notificação de ritmo alto, trocar:

```rust
                    &format!(
                        "{}/h nas últimas 3h ({} no período).",
                        usd(s.rate_per_hour),
                        usd(s.window_usd)
                    ),
```

por:

```rust
                    &format!(
                        "{}/h nas últimas {}h ({} no período).",
                        usd(s.rate_per_hour),
                        hours(s.window_hours),
                        usd(s.window_usd)
                    ),
```

- [ ] **Step 5: Alterar `main.rs`**

Logo abaixo de `mod usage;`, trocar:

```rust
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
```

por:

```rust
use settings::Settings;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
```

Substituir o `struct AppState { ... }` por:

```rust
struct AppState {
    snapshot: Mutex<Snapshot>,
    settings: Mutex<Settings>,
    settings_path: Option<PathBuf>,
    team: Mutex<Option<i64>>,
    /// Cache do gasto (centavos) de dias já encerrados, por data (AAAA-MM-DD).
    days: Mutex<std::collections::HashMap<String, f64>>,
    notify: Notify,
}

impl AppState {
    fn new() -> Self {
        let settings_path = settings::default_path();
        let settings = settings::load_or_init(settings_path.as_deref());
        AppState {
            snapshot: Mutex::new(Snapshot::empty("loading", None, &settings)),
            settings: Mutex::new(settings),
            settings_path,
            team: Mutex::new(None),
            days: Mutex::new(Default::default()),
            notify: Notify::new(),
        }
    }
}
```

Substituir `run_once` por:

```rust
async fn run_once(client: &reqwest::Client, state: &AppState) -> Snapshot {
    let settings = state.settings.lock().unwrap().clone();
    let Some(cookie) = config::load_cookie() else {
        return Snapshot::empty("auth", Some("Cole o cookie WorkosCursorSessionToken".into()), &settings);
    };
    let mut team = *state.team.lock().unwrap();
    let mut days = std::mem::take(&mut *state.days.lock().unwrap());
    let result = usage::fetch(client, &cookie, &settings, &mut team, &mut days).await;
    *state.team.lock().unwrap() = team;
    *state.days.lock().unwrap() = days;
    match result {
        Ok(s) => s,
        Err(FetchError::Auth) => Snapshot::empty(
            "auth",
            Some("Sessão expirada: cole o cookie novamente".into()),
            &settings,
        ),
        Err(FetchError::Other(msg)) => {
            // Mantém os últimos números bons e só sinaliza o erro.
            let mut prev = state.snapshot.lock().unwrap().clone();
            prev.status = "error".into();
            prev.message = Some(msg);
            prev
        }
    }
}
```

Substituir `poll_loop` por:

```rust
async fn poll_loop(app: tauri::AppHandle, state: Arc<AppState>) {
    let client = http_client();
    let mut alerts = alerts::Alerts::default();
    loop {
        let snap = run_once(&client, &state).await;
        *state.snapshot.lock().unwrap() = snap.clone();
        let _ = app.emit("usage", &snap);
        alerts.check(&app, &snap);
        let interval = state.settings.lock().unwrap().poll_seconds.max(10.0);
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs_f64(interval)) => {}
            _ = state.notify.notified() => {}
        }
    }
}
```

Em `main()`, no bloco `--once`, trocar:

```rust
            let state = AppState {
                snapshot: Mutex::new(Snapshot::empty("loading", None)),
                team: Mutex::new(None),
                days: Mutex::new(Default::default()),
                notify: Notify::new(),
            };
```

por:

```rust
            let state = AppState::new();
```

E trocar:

```rust
    let state = Arc::new(AppState {
        snapshot: Mutex::new(Snapshot::empty("loading", None)),
        team: Mutex::new(None),
        days: Mutex::new(Default::default()),
        notify: Notify::new(),
    });
```

por:

```rust
    let state = Arc::new(AppState::new());
```

- [ ] **Step 6: Rodar os testes**

Run: `cargo test`
Expected: `test result: ok. 13 passed`.

- [ ] **Step 7: Conferir o snapshot e o arquivo criado**

Run: `cargo run -- --once`
Expected: JSON com `"windowHours": 3.0`, `"lowMax"`, `"highMin"`, `"rateMax"` e `"dayMaxUsd"` iguais aos do `.env` (ou aos padrões), e `cycleLimitUsd` igual ao limite do Cursor.

Run: `Get-Content "$env:APPDATA\dev.local.cursor-usage-widget\settings.json"`
Expected: arquivo existe, com `"quotaMode": "auto"` e os mesmos números.

- [ ] **Step 8: Commit**

```powershell
git add src-tauri/src/usage.rs src-tauri/src/alerts.rs src-tauri/src/main.rs
git commit -m "feat: ritmo, cota, escalas e intervalo vindos das configuracoes"
```

---

### Task 3: Janela de Configurações (comandos + formulário)

**Files:**
- Modify: `src-tauri/src/main.rs` (comandos, `show_settings`, item da bandeja, `invoke_handler`)
- Modify: `src-tauri/capabilities/default.json`
- Create: `ui/settings.html`, `ui/settings.js`, `ui/settings.css`

**Interfaces:**
- Consumes: `Settings`, `SaveError`, `FieldError`, `settings::save`, `AppState.settings`, `AppState.settings_path`, `AppState.notify`.
- Produces (comandos Tauri, chamados pelo JS):
  - `get_settings() -> Settings`
  - `default_settings() -> Settings`
  - `save_settings({ settings }) -> Settings` ou erro `{ message: string, fields: [{ field, message }] }`
  - `open_settings()` — abre ou foca a janela `settings`
  - evento `settings` com o objeto `Settings` (JSON camelCase) após salvar

- [ ] **Step 1: Comandos e janela no `main.rs`**

Trocar `use settings::Settings;` por:

```rust
use settings::{SaveError, Settings};
```

Depois do comando `set_expanded`, adicionar:

```rust
#[tauri::command]
fn get_settings(state: State<'_, Arc<AppState>>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn default_settings() -> Settings {
    Settings::default()
}

#[tauri::command]
fn save_settings(
    app: tauri::AppHandle,
    settings: Settings,
    state: State<'_, Arc<AppState>>,
) -> Result<Settings, SaveError> {
    if let Err(fields) = settings.validate() {
        return Err(SaveError { message: "Corrija os campos destacados.".into(), fields });
    }
    let path = state.settings_path.as_ref().ok_or_else(|| SaveError {
        message: "Pasta de configurações (APPDATA) não encontrada.".into(),
        fields: Vec::new(),
    })?;
    settings::save(path, &settings).map_err(|message| SaveError { message, fields: Vec::new() })?;
    *state.settings.lock().unwrap() = settings.clone();
    let _ = app.emit("settings", &settings);
    state.notify.notify_one();
    Ok(settings)
}

/// Abre a janela de configurações ou a traz para a frente se já estiver aberta.
fn show_settings(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let _ = tauri::WebviewWindowBuilder::new(app, "settings", tauri::WebviewUrl::App("settings.html".into()))
        .title("Configurações · Cursor Usage")
        .inner_size(420.0, 640.0)
        .resizable(false)
        .maximizable(false)
        .center()
        .build();
}

// Assíncrono: criar janela num comando síncrono trava no Windows.
#[tauri::command]
async fn open_settings(app: tauri::AppHandle) {
    show_settings(&app);
}
```

No `invoke_handler`, trocar:

```rust
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            refresh_now,
            set_cookie,
            set_expanded
        ])
```

por:

```rust
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            refresh_now,
            set_cookie,
            set_expanded,
            get_settings,
            default_settings,
            save_settings,
            open_settings
        ])
```

No `setup`, trocar:

```rust
            let refresh = MenuItem::with_id(app, "refresh", "Atualizar agora", true, None::<&str>)?;
```

por:

```rust
            let refresh = MenuItem::with_id(app, "refresh", "Atualizar agora", true, None::<&str>)?;
            let open_cfg = MenuItem::with_id(app, "settings", "Configurações…", true, None::<&str>)?;
```

Trocar:

```rust
            let menu = Menu::with_items(app, &[&refresh, &toggle, &auto, &quit])?;
```

por:

```rust
            let menu = Menu::with_items(app, &[&refresh, &open_cfg, &toggle, &auto, &quit])?;
```

E, no `on_menu_event`, logo depois de `"refresh" => st.notify.notify_one(),`, adicionar:

```rust
                    "settings" => show_settings(app),
```

- [ ] **Step 2: Permissões**

Substituir `src-tauri/capabilities/default.json` por:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Permissões das janelas do widget e de configurações",
  "windows": ["main", "settings"],
  "permissions": ["core:default", "core:window:allow-close", "notification:default"]
}
```

- [ ] **Step 3: `ui/settings.html`**

```html
<!doctype html>
<html lang="pt-BR">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Configurações · Cursor Usage</title>
  <link rel="stylesheet" href="settings.css" />
</head>
<body>
  <form id="form" novalidate>
    <h1>Configurações</h1>

    <fieldset>
      <legend>Cota mensal</legend>
      <div class="seg-toggle">
        <label><input type="radio" name="quotaMode" value="auto" /> Automático (do Cursor)</label>
        <label><input type="radio" name="quotaMode" value="manual" /> Manual</label>
      </div>
      <label class="field" data-field="quotaUsd">
        <span>Valor da cota (US$)</span>
        <input name="quotaUsd" type="number" step="any" min="0" />
        <small class="err"></small>
      </label>
    </fieldset>

    <fieldset>
      <legend>Ritmo</legend>
      <label class="field" data-field="windowHours">
        <span>Janela de cálculo (horas)</span>
        <input name="windowHours" type="number" step="0.5" min="0.5" max="24" />
        <small class="err"></small>
      </label>
      <label class="field" data-field="rateMax">
        <span>Fim da escala do velocímetro (US$/h)</span>
        <input name="rateMax" type="number" step="any" min="0" />
        <small class="err"></small>
      </label>
      <label class="field" data-field="lowMax">
        <span>Baixo até (US$/h)</span>
        <input name="lowMax" type="number" step="any" min="0" />
        <small class="err"></small>
      </label>
      <label class="field" data-field="highMin">
        <span>Alto a partir de (US$/h)</span>
        <input name="highMin" type="number" step="any" min="0" />
        <small class="err"></small>
      </label>
    </fieldset>

    <fieldset>
      <legend>Semana</legend>
      <label class="field" data-field="dayMaxUsd">
        <span>Escala diária do equalizador (US$)</span>
        <input name="dayMaxUsd" type="number" step="any" min="1" />
        <small class="err"></small>
      </label>
    </fieldset>

    <fieldset>
      <legend>Geral</legend>
      <label class="field" data-field="pollSeconds">
        <span>Atualizar a cada (segundos)</span>
        <input name="pollSeconds" type="number" step="1" min="10" />
        <small class="err"></small>
      </label>
    </fieldset>

    <fieldset>
      <legend>Cores</legend>
      <div class="colors">
        <label class="field" data-field="colors.accent">
          <span>Destaque</span>
          <input name="colors.accent" type="color" />
          <small class="err"></small>
        </label>
        <label class="field" data-field="colors.low">
          <span>Baixo</span>
          <input name="colors.low" type="color" />
          <small class="err"></small>
        </label>
        <label class="field" data-field="colors.mid">
          <span>Médio</span>
          <input name="colors.mid" type="color" />
          <small class="err"></small>
        </label>
        <label class="field" data-field="colors.high">
          <span>Alto</span>
          <input name="colors.high" type="color" />
          <small class="err"></small>
        </label>
      </div>
      <button type="button" id="reset-colors" class="link">Restaurar cores padrão</button>
    </fieldset>

    <p id="form-msg" class="form-msg" hidden></p>

    <footer>
      <button type="button" id="reset-all" class="link">Restaurar tudo</button>
      <span class="spacer"></span>
      <button type="button" id="cancel" class="secondary">Cancelar</button>
      <button type="submit" class="primary">Salvar</button>
    </footer>
  </form>
  <script src="settings.js"></script>
</body>
</html>
```

- [ ] **Step 4: `ui/settings.js`**

```js
const { invoke } = window.__TAURI__.core;
const { getCurrentWindow } = window.__TAURI__.window;

const form = document.getElementById("form");
const NUMBER_FIELDS = ["quotaUsd", "windowHours", "rateMax", "lowMax", "highMin", "dayMaxUsd", "pollSeconds"];
const COLOR_KEYS = ["accent", "low", "mid", "high"];
let defaults = null;

const input = (name) => form.elements.namedItem(name);
const quotaMode = () => form.querySelector('input[name="quotaMode"]:checked').value;

function syncQuotaMode() {
  input("quotaUsd").disabled = quotaMode() !== "manual";
}

function fill(s, { colorsOnly = false } = {}) {
  if (!colorsOnly) {
    form.querySelector(`input[name="quotaMode"][value="${s.quotaMode}"]`).checked = true;
    NUMBER_FIELDS.forEach((k) => (input(k).value = s[k]));
    syncQuotaMode();
  }
  COLOR_KEYS.forEach((k) => (input(`colors.${k}`).value = s.colors[k]));
}

function read() {
  const s = { quotaMode: quotaMode(), colors: {} };
  NUMBER_FIELDS.forEach((k) => (s[k] = input(k).valueAsNumber));
  COLOR_KEYS.forEach((k) => (s.colors[k] = input(`colors.${k}`).value));
  return s;
}

function showErrors(fields, message) {
  form.querySelectorAll(".field").forEach((el) => {
    el.classList.remove("invalid");
    el.querySelector(".err").textContent = "";
  });
  for (const { field, message: m } of fields) {
    const el = form.querySelector(`.field[data-field="${field}"]`);
    if (!el) continue;
    el.classList.add("invalid");
    el.querySelector(".err").textContent = m;
  }
  const msg = document.getElementById("form-msg");
  msg.hidden = !message;
  msg.textContent = message ?? "";
}

form.addEventListener("change", (e) => {
  if (e.target.name === "quotaMode") syncQuotaMode();
});

form.addEventListener("submit", async (e) => {
  e.preventDefault();
  const s = read();
  // Campo vazio vira NaN, que o Rust não aceita como número.
  const missing = NUMBER_FIELDS.filter((k) => !Number.isFinite(s[k])).map((field) => ({
    field,
    message: "Informe um número.",
  }));
  if (missing.length) return showErrors(missing, "Corrija os campos destacados.");
  try {
    await invoke("save_settings", { settings: s });
    await getCurrentWindow().close();
  } catch (err) {
    showErrors(err?.fields ?? [], err?.message ?? String(err));
  }
});

document.getElementById("cancel").addEventListener("click", () => getCurrentWindow().close());
document.getElementById("reset-colors").addEventListener("click", () => fill(defaults, { colorsOnly: true }));
document.getElementById("reset-all").addEventListener("click", () => {
  fill(defaults);
  showErrors([], "");
});

Promise.all([invoke("get_settings"), invoke("default_settings")]).then(([s, d]) => {
  defaults = d;
  fill(s);
});
```

- [ ] **Step 5: `ui/settings.css`**

```css
:root {
  --black: #000;
  --g1: #0b0b0c;
  --g2: #1c1c1e;
  --g3: #2c2c2e;
  --g4: #48484a;
  --g5: #8e8e93;
  --text: #f5f5f7;
  --orange: #ff9f0a;
  --orange-deep: #ff6a00;
  --red: #ff453a;
}

* { box-sizing: border-box; margin: 0; padding: 0; }
html, body { background: var(--g1); color: var(--text); }
body {
  font-family: "Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif;
  font-size: 13px;
  -webkit-font-smoothing: antialiased;
}

form { display: flex; flex-direction: column; gap: 14px; min-height: 100vh; padding: 20px 22px 0; }
h1 { font-size: 18px; font-weight: 600; letter-spacing: -.3px; }

fieldset {
  display: grid; gap: 10px; padding: 12px 14px 14px;
  border: 1px solid var(--g3); border-radius: 14px;
  background: linear-gradient(180deg, var(--g2), var(--g1));
}
legend { padding: 0 6px; font-size: 10.5px; font-weight: 600; color: var(--g5); text-transform: uppercase; letter-spacing: 1.3px; }

.field { display: grid; grid-template-columns: 1fr 110px; align-items: center; column-gap: 10px; }
.field input[type="number"] {
  height: 30px; padding: 0 10px; text-align: right; outline: none;
  background: var(--black); border: 1px solid var(--g4); border-radius: 8px;
  color: var(--text); font: inherit;
}
.field input:focus { border-color: var(--orange); box-shadow: 0 0 0 3px color-mix(in srgb, var(--orange) 18%, transparent); }
.field input:disabled { opacity: .4; }
.field .err { grid-column: 1 / -1; margin-top: 4px; color: var(--red); font-size: 11px; }
.field .err:empty { display: none; }
.field.invalid input { border-color: var(--red); }

.seg-toggle { display: flex; gap: 14px; }
.seg-toggle label { display: flex; align-items: center; gap: 6px; cursor: pointer; }
input[type="radio"] { accent-color: var(--orange); }

.colors { display: grid; grid-template-columns: 1fr 1fr; gap: 10px 16px; }
.colors .field { grid-template-columns: 1fr 44px; }
.field input[type="color"] {
  width: 44px; height: 28px; padding: 2px; cursor: pointer;
  background: var(--black); border: 1px solid var(--g4); border-radius: 8px;
}

.link { justify-self: start; background: none; border: none; color: var(--orange); font: inherit; font-size: 12px; cursor: pointer; }
.link:hover { text-decoration: underline; }

.form-msg { color: var(--red); font-size: 12px; }

footer {
  position: sticky; bottom: 0; margin-top: auto;
  display: flex; align-items: center; gap: 8px; padding: 14px 0 18px;
  background: linear-gradient(180deg, transparent, var(--g1) 30%);
}
.spacer { flex: 1; }
.primary, .secondary { height: 32px; padding: 0 16px; border-radius: 10px; font: inherit; font-weight: 600; cursor: pointer; }
.primary { border: none; background: linear-gradient(180deg, var(--orange), var(--orange-deep)); color: #000; }
.secondary { border: 1px solid var(--g4); background: var(--g2); color: var(--text); }
.primary:hover, .secondary:hover { filter: brightness(1.1); }
.primary:active, .secondary:active { transform: scale(.97); }

[hidden] { display: none !important; }
```

- [ ] **Step 6: Compilar**

Run: `cargo build`
Expected: compila sem erros.

- [ ] **Step 7: Teste manual da janela**

Run: `cargo run` (deixar rodando).

1. Bandeja › "Configurações…": a janela abre centralizada com os valores atuais; o campo "Valor da cota" fica desabilitado em Automático.
2. Bandeja › "Configurações…" de novo: não abre uma segunda janela, só foca a existente.
3. Pôr "Baixo até" = 15 e Salvar: aparece "Baixo precisa ser menor que Alto." sob o campo, borda vermelha, a janela continua aberta e `settings.json` não muda.
4. Apagar "Atualizar a cada" e Salvar: aparece "Informe um número.".
5. "Restaurar tudo" e depois mudar "Atualizar a cada" para 15 e Salvar: a janela fecha; `Get-Content "$env:APPDATA\dev.local.cursor-usage-widget\settings.json"` mostra `"pollSeconds": 15.0`.
6. Abrir de novo, mudar algo e clicar Cancelar: a janela fecha e o arquivo não muda.

- [ ] **Step 8: Commit**

```powershell
git add src-tauri/src/main.rs src-tauri/capabilities/default.json ui/settings.html ui/settings.js ui/settings.css
git commit -m "feat: janela de configuracoes (bandeja) com validacao por campo"
```

---

### Task 4: Widget aplica cores e janela configuradas

**Files:**
- Modify: `ui/style.css` (variáveis de nível, `color-mix`, engrenagem)
- Modify: `ui/index.html` (paradas do degradê, rótulos da janela, botão de engrenagem)
- Modify: `ui/app.js` (`applyColors`, rótulo da janela, cor base do ciclo, engrenagem, evento `settings`)

**Interfaces:**
- Consumes: comandos `get_settings`, `open_settings`; evento `settings`; campo `windowHours` do snapshot (Tarefas 2 e 3).
- Produces: variáveis CSS `--orange`, `--lvl-low`, `--lvl-mid`, `--lvl-high` no `:root`, definidas pelo JS.

- [ ] **Step 1: `ui/style.css`**

No `:root`, trocar:

```css
  --orange: #ff9f0a;
  --orange-deep: #ff6a00;
  --red: #ff453a;
```

por:

```css
  --orange: #ff9f0a;
  --orange-deep: oklch(from var(--orange) calc(l - 0.08) c calc(h - 20));
  --red: #ff453a;
  --lvl-low: #30d158;
  --lvl-mid: #ffd60a;
  --lvl-high: #ff453a;
```

Trocar as três regras de nível:

```css
.widget[data-level="low"]  { --accent: #30d158; --accent-bg: rgba(48, 209, 88, .16); }
.widget[data-level="mid"]  { --accent: #ffd60a; --accent-bg: rgba(255, 214, 10, .16); }
.widget[data-level="high"] { --accent: var(--red); --accent-bg: rgba(255, 69, 58, .16); }
```

por:

```css
.widget[data-level="low"]  { --accent: var(--lvl-low);  --accent-bg: color-mix(in srgb, var(--lvl-low) 16%, transparent); }
.widget[data-level="mid"]  { --accent: var(--lvl-mid);  --accent-bg: color-mix(in srgb, var(--lvl-mid) 16%, transparent); }
.widget[data-level="high"] { --accent: var(--lvl-high); --accent-bg: color-mix(in srgb, var(--lvl-high) 16%, transparent); }
```

No `.widget`, trocar:

```css
    radial-gradient(130% 70% at 85% 0%, rgba(255, 159, 10, 0.10) 0%, transparent 55%),
```

por:

```css
    radial-gradient(130% 70% at 85% 0%, color-mix(in srgb, var(--orange) 10%, transparent) 0%, transparent 55%),
```

Trocar:

```css
.needle line { stroke: var(--orange); stroke-width: 2.6; stroke-linecap: round; filter: drop-shadow(0 0 3px rgba(255, 159, 10, .8)); }
```

por:

```css
.needle line { stroke: var(--orange); stroke-width: 2.6; stroke-linecap: round; filter: drop-shadow(0 0 3px color-mix(in srgb, var(--orange) 80%, transparent)); }
```

Trocar:

```css
.cookie input:focus { border-color: var(--orange); box-shadow: 0 0 0 3px rgba(255, 159, 10, .18); }
```

por:

```css
.cookie input:focus { border-color: var(--orange); box-shadow: 0 0 0 3px color-mix(in srgb, var(--orange) 18%, transparent); }
```

Logo depois de `.clock { ... }`, adicionar:

```css
.head-right { display: flex; align-items: center; gap: 8px; }
.gear { display: grid; place-items: center; width: 18px; height: 18px; padding: 0; border: none; background: none; color: var(--g4); cursor: pointer; }
.gear svg { width: 14px; height: 14px; fill: none; stroke: currentColor; stroke-width: 2; stroke-linecap: round; stroke-linejoin: round; }
.gear:hover { color: var(--orange); }
```

- [ ] **Step 2: `ui/index.html`**

Trocar as paradas do degradê:

```html
              <stop offset="0" stop-color="#30d158" />
              <stop offset="0.5" stop-color="#ffd60a" />
              <stop offset="0.78" stop-color="#ff9f0a" />
              <stop offset="1" stop-color="#ff453a" />
```

por:

```html
              <stop offset="0" style="stop-color: var(--lvl-low)" />
              <stop offset="0.5" style="stop-color: var(--lvl-mid)" />
              <stop offset="0.78" style="stop-color: var(--orange)" />
              <stop offset="1" style="stop-color: var(--lvl-high)" />
```

Trocar o cabeçalho:

```html
      <header class="row r1">
        <span class="eyebrow">Ritmo · 3h</span>
        <span id="clock" class="clock"></span>
      </header>
```

por:

```html
      <header class="row r1">
        <span class="eyebrow">Ritmo · <span id="window-label">3h</span></span>
        <span class="head-right">
          <span id="clock" class="clock"></span>
          <button id="open-settings" class="gear" type="button" title="Configurações" aria-label="Configurações">
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <circle cx="12" cy="12" r="3" />
              <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z" />
            </svg>
          </button>
        </span>
      </header>
```

Trocar:

```html
          <span class="muted">últimas 3h</span> <b id="window">--</b>
```

por:

```html
          <span class="muted">últimas <span id="window-hours">3h</span></span> <b id="window">--</b>
```

- [ ] **Step 3: `ui/app.js`**

Depois de `const LEVEL_LABEL = { ... };`, adicionar:

```js
const hoursLabel = (h) => (Number.isInteger(h) ? String(h) : h.toFixed(1).replace(".", ",")) + "h";

function applyColors(c) {
  const st = document.documentElement.style;
  st.setProperty("--orange", c.accent);
  st.setProperty("--lvl-low", c.low);
  st.setProperty("--lvl-mid", c.mid);
  st.setProperty("--lvl-high", c.high);
}
```

Trocar:

```js
const cycleColor = (pct) =>
  pct >= 90 ? "#ff453a" : pct >= 75 ? "#ff6a00" : "#ff9f0a";
```

por:

```js
const cycleColor = (pct) =>
  pct >= 90 ? "#ff453a" : pct >= 75 ? "#ff6a00" : "var(--orange)";
```

Em `render`, logo antes de `// velocímetro`, adicionar:

```js
  const wh = hoursLabel(s.windowHours ?? 3);
  $("window-label").textContent = wh;
  $("window-hours").textContent = wh;

```

Depois de `root.addEventListener("dblclick", () => invoke("refresh_now"));`, adicionar:

```js
$("open-settings").addEventListener("click", () => invoke("open_settings"));
$("open-settings").addEventListener("dblclick", (e) => e.stopPropagation());
```

Na seção `// ---------- inicialização ----------`, depois de `listen("usage", ...)`, adicionar:

```js
listen("settings", (ev) => applyColors(ev.payload.colors));
invoke("get_settings").then((s) => applyColors(s.colors));
```

- [ ] **Step 4: Teste manual do widget**

Run: `cargo run` (deixar rodando).

1. Com as cores padrão, passar o mouse no widget e tirar um print: `powershell -File tools/shot.ps1 -Out "$env:TEMP\cuw\cores-padrao.png" -Y 300 -H 520`. Conferir que o visual continua o de antes (laranja, LED verde/amarelo/vermelho, degradê do velocímetro) e que o cabeçalho mostra "Ritmo · 3h" e a engrenagem.
2. Clicar na engrenagem: abre a janela de Configurações.
3. Mudar Destaque para `#0a84ff`, Baixo para `#64d2ff`, "Janela de cálculo" para 1 e Salvar. Sem reiniciar, o widget troca agulha, botão e realces para azul na hora; após a nova consulta, mostra "Ritmo · 1h" e "últimas 1h".
4. Print: `powershell -File tools/shot.ps1 -Out "$env:TEMP\cuw\cores-azul.png" -Y 300 -H 520` e conferir.
5. Fechar pela bandeja (Sair) e rodar `cargo run` de novo: as cores azuis e a janela de 1h continuam.
6. Configurações › "Restaurar tudo" › Salvar: o widget volta ao laranja e a 3h.

- [ ] **Step 5: Commit**

```powershell
git add ui/style.css ui/index.html ui/app.js
git commit -m "feat(ui): cores e janela do ritmo configuraveis, engrenagem no widget"
```

---

### Task 5: Verificação final

**Files:** nenhum (só verificação).

**Interfaces:**
- Consumes: tudo das tarefas anteriores.
- Produces: nada.

- [ ] **Step 1: Testes e build de release**

Run: `cargo test`
Expected: `test result: ok. 13 passed`.

Run: `cargo build --release`
Expected: compila sem erros.

- [ ] **Step 2: Snapshot com cota manual**

Abrir Configurações, escolher Manual com 250 e Salvar. Fechar o app.

Run: `cargo run -- --once`
Expected: `"cycleLimitUsd": 250.0` e `"windowHours"` igual ao configurado.

Voltar para Automático pelo painel e conferir com `cargo run -- --once` que `cycleLimitUsd` volta ao limite do Cursor.

- [ ] **Step 3: Arquivo corrompido não derruba o app**

```powershell
$f = "$env:APPDATA\dev.local.cursor-usage-widget\settings.json"
Copy-Item $f "$f.bak"
Set-Content $f '{ "lowMax": 99, "colors": { "accent": "azul" } '
cargo run -- --once
Move-Item -Force "$f.bak" $f
```

Expected: o `--once` imprime o snapshot normalmente, com `lowMax` 6, `highMin` 13 e `rateMax` 20 (padrões).

- [ ] **Step 4: Notificação com a janela configurada**

Run: `cargo run -- --test-notify`
Expected: aparece o toast de teste (confirma que as notificações continuam funcionando). A mensagem de ritmo alto ("nas últimas Xh") é coberta pela leitura do código em `alerts.rs`; não há como forçá-la sem gasto real.
