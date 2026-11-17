#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod usage;

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager, PhysicalPosition, PhysicalSize, State, WebviewWindow,
};
use tokio::sync::Notify;
use usage::{FetchError, Snapshot};

// Tamanhos lógicos (DIPs) da janela: recolhida (aba) e expandida (painel).
const COLLAPSED: (f64, f64) = (56.0, 112.0);
const EXPANDED: (f64, f64) = (272.0, 316.0);

struct AppState {
    snapshot: Mutex<Snapshot>,
    team: Mutex<Option<i64>>,
    notify: Notify,
}

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) CursorUsageWidget/0.1")
        .build()
        .expect("cliente HTTP")
}

async fn run_once(client: &reqwest::Client, state: &AppState) -> Snapshot {
    let Some(cookie) = config::load_cookie() else {
        return Snapshot::empty("auth", Some("Cole o cookie WorkosCursorSessionToken".into()));
    };
    let mut team = *state.team.lock().unwrap();
    let result = usage::fetch(client, &cookie, &mut team).await;
    *state.team.lock().unwrap() = team;
    match result {
        Ok(s) => s,
        Err(FetchError::Auth) => {
            Snapshot::empty("auth", Some("Sessão expirada: cole o cookie novamente".into()))
        }
        Err(FetchError::Other(msg)) => {
            // Mantém os últimos números bons e só sinaliza o erro.
            let mut prev = state.snapshot.lock().unwrap().clone();
            prev.status = "error".into();
            prev.message = Some(msg);
            prev
        }
    }
}

async fn poll_loop(app: tauri::AppHandle, state: Arc<AppState>) {
    let client = http_client();
    let interval = config::setting_f64("POLL_SECONDS", 30.0).max(10.0);
    loop {
        let snap = run_once(&client, &state).await;
        *state.snapshot.lock().unwrap() = snap.clone();
        let _ = app.emit("usage", &snap);
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

fn main() {
    // Modo diagnóstico: `cursor-usage-widget --once` imprime o snapshot e sai.
    if std::env::args().any(|a| a == "--once") {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        rt.block_on(async {
            let state = AppState {
                snapshot: Mutex::new(Snapshot::empty("loading", None)),
                team: Mutex::new(None),
                notify: Notify::new(),
            };
            let snap = run_once(&http_client(), &state).await;
            println!("{}", serde_json::to_string_pretty(&snap).unwrap());
        });
        return;
    }

    let state = Arc::new(AppState {
        snapshot: Mutex::new(Snapshot::empty("loading", None)),
        team: Mutex::new(None),
        notify: Notify::new(),
    });

    tauri::Builder::default()
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            refresh_now,
            set_cookie,
            set_expanded
        ])
        .setup(move |app| {
            let win = app.get_webview_window("main").expect("janela main");
            place(&win, false);

            let refresh = MenuItem::with_id(app, "refresh", "Atualizar agora", true, None::<&str>)?;
            let toggle = MenuItem::with_id(app, "toggle", "Mostrar/ocultar", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Sair", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&refresh, &toggle, &quit])?;
            let st = state.clone();
            TrayIconBuilder::new()
                .icon(app.default_window_icon().expect("ícone").clone())
                .tooltip("Cursor Usage")
                .menu(&menu)
                .on_menu_event(move |app, ev| match ev.id.as_ref() {
                    "refresh" => st.notify.notify_one(),
                    "toggle" => {
                        if let Some(w) = app.get_webview_window("main") {
                            if w.is_visible().unwrap_or(true) {
                                let _ = w.hide();
                            } else {
                                let _ = w.show();
                            }
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            tauri::async_runtime::spawn(poll_loop(app.handle().clone(), state.clone()));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("erro ao executar o app");
}
