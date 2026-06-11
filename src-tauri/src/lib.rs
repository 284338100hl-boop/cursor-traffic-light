use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, WindowEvent};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_store::StoreExt;

const STORE_FILE: &str = "settings.json";
const IDLE_MS_KEY: &str = "idle_ms";
const NOTIFY_ON_COMPLETE_KEY: &str = "notify_on_complete";
const ALWAYS_ON_TOP_KEY: &str = "always_on_top";
const DEFAULT_IDLE_MS: u64 = 30_000;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrafficLightState {
    pub light: String,
    pub updated_at: u64,
    #[serde(default)]
    pub blink: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub idle_ms: u64,
    pub notify_on_complete: bool,
    pub always_on_top: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            idle_ms: DEFAULT_IDLE_MS,
            notify_on_complete: false,
            always_on_top: true,
        }
    }
}

impl TrafficLightState {
    fn idle() -> Self {
        Self {
            light: "green".into(),
            updated_at: 0,
            blink: None,
            source: Some("default".into()),
            label: None,
        }
    }
}

pub fn resolve_state_path() -> PathBuf {
    if let Ok(custom) = std::env::var("AGENT_TRAFFIC_LIGHT_STATE") {
        if !custom.is_empty() {
            return PathBuf::from(custom);
        }
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cursor")
        .join("agent-traffic-light-state.json")
}

fn read_state_file(path: &PathBuf) -> TrafficLightState {
    let raw = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return TrafficLightState::idle(),
    };
    match serde_json::from_str::<TrafficLightState>(&raw) {
        Ok(s) if matches!(s.light.as_str(), "red" | "yellow" | "green") => s,
        _ => TrafficLightState::idle(),
    }
}

fn emit_state(app: &AppHandle, path: &PathBuf) {
    let state = read_state_file(path);
    let _ = app.emit("traffic-light-changed", state);
}

fn load_settings(app: &AppHandle) -> AppSettings {
    if let Ok(store) = app.store(STORE_FILE) {
        let idle_ms = store
            .get(IDLE_MS_KEY)
            .and_then(|v| v.as_u64())
            .unwrap_or(DEFAULT_IDLE_MS);
        let notify_on_complete = store
            .get(NOTIFY_ON_COMPLETE_KEY)
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let always_on_top = store
            .get(ALWAYS_ON_TOP_KEY)
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        return AppSettings {
            idle_ms,
            notify_on_complete,
            always_on_top,
        };
    }
    AppSettings::default()
}

fn save_settings(app: &AppHandle, settings: &AppSettings) {
    if let Ok(store) = app.store(STORE_FILE) {
        let _ = store.set(IDLE_MS_KEY, settings.idle_ms);
        let _ = store.set(NOTIFY_ON_COMPLETE_KEY, settings.notify_on_complete);
        let _ = store.set(ALWAYS_ON_TOP_KEY, settings.always_on_top);
        let _ = store.save();
    }
}

fn maybe_notify_completion(app: &AppHandle, prev: &str, current: &str) {
    if prev != "green" && current == "green" {
        let settings = load_settings(app);
        if settings.notify_on_complete {
            let _ = app.notification().builder()
                .title("Cursor Agent")
                .body("任务完成")
                .show();
        }
    }
}

fn start_state_watcher(app: AppHandle, path: PathBuf, last_light: Arc<Mutex<String>>) {
    let parent = path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    std::thread::spawn(move || {
        let (tx, rx) = mpsc::channel();
        let mut watcher = match RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            Config::default(),
        ) {
            Ok(w) => w,
            Err(_) => return,
        };

        if watcher
            .watch(&parent, RecursiveMode::NonRecursive)
            .is_err()
        {
            return;
        }

        emit_state(&app, &path);

        loop {
            match rx.recv_timeout(Duration::from_millis(400)) {
                Ok(Ok(event)) => {
                    use notify::EventKind;
                    match event.kind {
                        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => {
                            if event.paths.iter().any(|p| p == &path) {
                                let new_state = read_state_file(&path);
                                let mut last = last_light.lock().unwrap();
                                maybe_notify_completion(&app, &last, &new_state.light);
                                *last = new_state.light.clone();
                                drop(last);
                                emit_state(&app, &path);
                            }
                        }
                        _ => {}
                    }
                }
                Ok(Err(_)) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });
}

#[tauri::command]
fn get_traffic_light_state() -> TrafficLightState {
    read_state_file(&resolve_state_path())
}

#[tauri::command]
fn get_state_file_path() -> String {
    resolve_state_path().to_string_lossy().into_owned()
}

#[tauri::command]
fn get_settings(app: AppHandle) -> AppSettings {
    load_settings(&app)
}

#[tauri::command]
fn set_settings(app: AppHandle, settings: AppSettings) {
    save_settings(&app, &settings);
    // 应用 always_on_top 设置到主窗口
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_always_on_top(settings.always_on_top);
    }
    let _ = app.emit("settings-changed", settings);
}

#[tauri::command]
fn show_settings_window(app: AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let last_light = Arc::new(Mutex::new("green".to_string()));

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(move |app| {
            let path = resolve_state_path();
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }

            start_state_watcher(app.handle().clone(), path, last_light.clone());

            // 应用启动时加载并应用 always_on_top 设置
            let settings = load_settings(app.handle());
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_always_on_top(settings.always_on_top);
            }

            // 设置窗口关闭时不退出，只隐藏
            if let Some(window) = app.get_webview_window("settings") {
                let window_handle = window.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window_handle.hide();
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_traffic_light_state,
            get_state_file_path,
            get_settings,
            set_settings,
            show_settings_window
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}