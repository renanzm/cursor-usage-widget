#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod alerts;
mod config;
mod settings;
mod usage;

use settings::{SaveError, Settings};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager, PhysicalPosition, PhysicalSize, State, WebviewWindow,
};
use tokio::sync::Notify;
use usage::{FetchError, Snapshot};

// Tamanhos lógicos (DIPs) da janela: recolhida (aba) e expandida (painel).
const COLLAPSED: (f64, f64) = (56.0, 112.0);
const EXPANDED: (f64, f64) = (272.0, 490.0);

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

// ---------- iniciar com o Windows (chave HKCU\...\Run) ----------
const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_NAME: &str = "CursorUsageWidget";

fn reg(args: &[&str]) -> bool {
    use std::os::windows::process::CommandExt;
    std::process::Command::new("reg")
        .args(args)
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn autostart_enabled() -> bool {
    reg(&["query", RUN_KEY, "/v", RUN_NAME])
}

fn set_autostart(on: bool) {
    if on {
        if let Ok(exe) = std::env::current_exe() {
            let value = format!("\"{}\"", exe.display());
            reg(&["add", RUN_KEY, "/v", RUN_NAME, "/t", "REG_SZ", "/d", &value, "/f"]);
        }
    } else {
        reg(&["delete", RUN_KEY, "/v", RUN_NAME, "/f"]);
    }
}

/// Registra o AppUserModelID para o Windows aceitar exibir toasts deste executável.
fn register_toast_identity(identifier: &str) {
    let key = format!(r"HKCU\Software\Classes\AppUserModelId\{identifier}");
    reg(&["add", &key, "/v", "DisplayName", "/t", "REG_SZ", "/d", "Cursor Usage Widget", "/f"]);
}

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) CursorUsageWidget/0.1")
        .build()
        .expect("cliente HTTP")
}

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
            // Mantém os últimos números bons e só sinaliza o erro; as escalas seguem as configurações atuais.
            let mut prev = state.snapshot.lock().unwrap().clone();
            prev.status = "error".into();
            prev.message = Some(msg);
            prev.low_max = settings.low_max;
            prev.high_min = settings.high_min;
            prev.rate_max = settings.rate_max;
            prev.day_max_usd = settings.day_max_usd;
            prev.level = usage::level_of(prev.rate_per_hour, prev.low_max, prev.high_min).into();
            if settings.quota_mode == settings::QuotaMode::Manual {
                prev.cycle_limit_usd = settings.quota_usd;
            }
            prev
        }
    }
}

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

/// Posiciona a janela colada na borda direita do monitor mais à direita, centrada na vertical.
fn place(win: &WebviewWindow, expanded: bool) {
    let monitors = win.available_monitors().unwrap_or_default();
    let Some(m) = monitors
        .iter()
        .max_by_key(|m| m.position().x + m.size().width as i32)
    else {
        return;
    };
    let scale = m.scale_factor();
    let (w, h) = if expanded { EXPANDED } else { COLLAPSED };
    let (pw, ph) = ((w * scale).round() as u32, (h * scale).round() as u32);
    let x = m.position().x + m.size().width as i32 - pw as i32;
    let y = m.position().y + (m.size().height as i32 - ph as i32) / 2;
    let _ = win.set_size(PhysicalSize::new(pw, ph));
    let _ = win.set_position(PhysicalPosition::new(x, y));
}

#[tauri::command]
fn get_snapshot(state: State<'_, Arc<AppState>>) -> Snapshot {
    state.snapshot.lock().unwrap().clone()
}

#[tauri::command]
fn refresh_now(state: State<'_, Arc<AppState>>) {
    state.notify.notify_one();
}

#[tauri::command]
fn set_cookie(cookie: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let normalized = config::normalize(&cookie).ok_or("Cookie vazio")?;
    config::save_cookie(&normalized)?;
    *state.team.lock().unwrap() = None;
    state.notify.notify_one();
    Ok(())
}

#[tauri::command]
fn set_expanded(window: WebviewWindow, expanded: bool) {
    place(&window, expanded);
}

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

fn main() {
    // Modo diagnóstico: `cursor-usage-widget --once` imprime o snapshot e sai.
    if std::env::args().any(|a| a == "--once") {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        rt.block_on(async {
            let state = AppState::new();
            let snap = run_once(&http_client(), &state).await;
            println!("{}", serde_json::to_string_pretty(&snap).unwrap());
        });
        return;
    }

    let state = Arc::new(AppState::new());

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .manage(state.clone())
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
        .setup(move |app| {
            register_toast_identity(&app.config().identifier);
            let win = app.get_webview_window("main").expect("janela main");
            place(&win, false);

            let refresh = MenuItem::with_id(app, "refresh", "Atualizar agora", true, None::<&str>)?;
            let open_cfg = MenuItem::with_id(app, "settings", "Configurações…", true, None::<&str>)?;
            let toggle = MenuItem::with_id(app, "toggle", "Mostrar/ocultar", true, None::<&str>)?;
            let auto = CheckMenuItem::with_id(
                app,
                "autostart",
                "Iniciar com o Windows",
                true,
                autostart_enabled(),
                None::<&str>,
            )?;
            let quit = MenuItem::with_id(app, "quit", "Sair", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&refresh, &open_cfg, &toggle, &auto, &quit])?;
            let st = state.clone();
            let auto_item = auto.clone();
            TrayIconBuilder::new()
                .icon(app.default_window_icon().expect("ícone").clone())
                .tooltip("Cursor Usage")
                .menu(&menu)
                .on_menu_event(move |app, ev| match ev.id.as_ref() {
                    "refresh" => st.notify.notify_one(),
                    "settings" => show_settings(app),
                    "toggle" => {
                        if let Some(w) = app.get_webview_window("main") {
                            if w.is_visible().unwrap_or(true) {
                                let _ = w.hide();
                            } else {
                                let _ = w.show();
                            }
                        }
                    }
                    "autostart" => {
                        // O item já alterna sozinho ao clicar; sincroniza o registro com ele.
                        let on = auto_item.is_checked().unwrap_or(false);
                        set_autostart(on);
                        let _ = auto_item.set_checked(autostart_enabled());
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            // `--test-notify` dispara uma notificação de teste ao abrir.
            if std::env::args().any(|a| a == "--test-notify") {
                alerts::notify(
                    app.handle(),
                    "Cursor Usage: teste",
                    "As notificações do widget estão funcionando.",
                );
            }

            tauri::async_runtime::spawn(poll_loop(app.handle().clone(), state.clone()));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("erro ao executar o app");
}
