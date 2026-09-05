use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, RunEvent, WindowEvent};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_store::StoreExt;

const STORE_FILE: &str = "settings.json";
const IDLE_MS_KEY: &str = "idle_ms";
const NOTIFY_ON_COMPLETE_KEY: &str = "notify_on_complete";
const NOTIFY_NTFY_KEY: &str = "notify_ntfy";
const NTFY_TOPIC_KEY: &str = "ntfy_topic";
const ALWAYS_ON_TOP_KEY: &str = "always_on_top";
const SHOW_HOVER_TOOLTIP_KEY: &str = "show_hover_tooltip";
const AUTO_LAUNCH_WITH_CURSOR_KEY: &str = "auto_launch_with_cursor";
const EXE_PATH_KEY: &str = "exe_path";
const DEFAULT_IDLE_MS: u64 = 30_000;
const NOTIFY_SETTLE: Duration = Duration::from_millis(1200);

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityTask {
    pub id: String,
    pub kind: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
}

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
    #[serde(default)]
    pub tasks: Vec<ActivityTask>,
    #[serde(default)]
    pub notify_complete: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub idle_ms: u64,
    pub notify_on_complete: bool,
    pub notify_ntfy: bool,
    pub ntfy_topic: String,
    pub always_on_top: bool,
    pub show_hover_tooltip: bool,
    pub auto_launch_with_cursor: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            idle_ms: DEFAULT_IDLE_MS,
            notify_on_complete: false,
            notify_ntfy: true,
            ntfy_topic: String::new(),
            always_on_top: true,
            show_hover_tooltip: true,
            auto_launch_with_cursor: false,
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
            tasks: Vec::new(),
            notify_complete: false,
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

fn pid_lock_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cursor")
        .join("agent-traffic-light.lock")
}

fn write_pid_lock() {
    let path = pid_lock_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, std::process::id().to_string());
}

fn remove_pid_lock() {
    let _ = std::fs::remove_file(pid_lock_path());
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
        let notify_ntfy = store
            .get(NOTIFY_NTFY_KEY)
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let ntfy_topic = store
            .get(NTFY_TOPIC_KEY)
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_default();
        let always_on_top = store
            .get(ALWAYS_ON_TOP_KEY)
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let show_hover_tooltip = store
            .get(SHOW_HOVER_TOOLTIP_KEY)
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let auto_launch_with_cursor = store
            .get(AUTO_LAUNCH_WITH_CURSOR_KEY)
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        return AppSettings {
            idle_ms,
            notify_on_complete,
            notify_ntfy,
            ntfy_topic,
            always_on_top,
            show_hover_tooltip,
            auto_launch_with_cursor,
        };
    }
    AppSettings::default()
}

fn save_settings(app: &AppHandle, settings: &AppSettings) {
    if let Ok(store) = app.store(STORE_FILE) {
        let _ = store.set(IDLE_MS_KEY, settings.idle_ms);
        let _ = store.set(NOTIFY_ON_COMPLETE_KEY, settings.notify_on_complete);
        let _ = store.set(NOTIFY_NTFY_KEY, settings.notify_ntfy);
        let _ = store.set(NTFY_TOPIC_KEY, settings.ntfy_topic.clone());
        let _ = store.set(ALWAYS_ON_TOP_KEY, settings.always_on_top);
        let _ = store.set(SHOW_HOVER_TOOLTIP_KEY, settings.show_hover_tooltip);
        let _ = store.set(AUTO_LAUNCH_WITH_CURSOR_KEY, settings.auto_launch_with_cursor);
        let _ = store.save();
    }
}

#[derive(Clone, Copy)]
enum PendingNotify {
    Complete,
    Attention,
}

fn classify_notify(prev: &str, new_state: &TrafficLightState) -> Option<PendingNotify> {
    if prev == new_state.light {
        return None;
    }
    let completed = prev != "green"
        && new_state.light == "green"
        && new_state.notify_complete
        && !new_state.tasks.iter().any(|t| t.kind == "subagent");
    if completed {
        return Some(PendingNotify::Complete);
    }
    if prev != "red" && new_state.light == "red" {
        return Some(PendingNotify::Attention);
    }
    None
}

fn send_notify(app: &AppHandle, kind: PendingNotify) {
    let settings = load_settings(app);
    match kind {
        PendingNotify::Complete => {
            if settings.notify_on_complete {
                let _ = app
                    .notification()
                    .builder()
                    .title("Cursor Agent")
                    .body("任务完成")
                    .show();
            }
            if settings.notify_ntfy {
                push_ntfy(&settings.ntfy_topic, "任务完成", "default");
            }
        }
        PendingNotify::Attention => {
            if settings.notify_ntfy {
                push_ntfy(&settings.ntfy_topic, "有问题需要处理", "high");
            }
        }
    }
}

fn ntfy_url(topic: &str) -> Option<String> {
    let topic = topic.trim();
    if topic.is_empty() {
        return None;
    }
    if !topic
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return None;
    }
    Some(format!("https://ntfy.sh/{topic}"))
}

fn push_ntfy(topic: &str, body: &str, priority: &str) {
    let Some(url) = ntfy_url(topic) else {
        return;
    };
    let body = body.to_string();
    let priority = priority.to_string();
    std::thread::spawn(move || {
        let _ = ureq::post(&url)
            .set("Title", "Cursor Agent")
            .set("Priority", &priority)
            .send_string(&body);
    });
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

        let mut pending: Option<(PendingNotify, Instant)> = None;

        loop {
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(Ok(event)) => {
                    use notify::EventKind;
                    match event.kind {
                        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => {
                            if event.paths.iter().any(|p| p == &path) {
                                let new_state = read_state_file(&path);
                                let mut last = last_light.lock().unwrap();
                                let classified = classify_notify(&last, &new_state);
                                *last = new_state.light.clone();
                                drop(last);
                                emit_state(&app, &path);

                                match classified {
                                    Some(kind) => {
                                        pending = Some((kind, Instant::now() + NOTIFY_SETTLE));
                                    }
                                    None => {
                                        if new_state.light != "green"
                                            && matches!(
                                                pending,
                                                Some((PendingNotify::Complete, _))
                                            )
                                        {
                                            pending = None;
                                        }
                                        if new_state.light != "red"
                                            && matches!(
                                                pending,
                                                Some((PendingNotify::Attention, _))
                                            )
                                        {
                                            pending = None;
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Ok(Err(_)) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if let Some((kind, due)) = pending {
                        if Instant::now() >= due {
                            pending = None;
                            let state = read_state_file(&path);
                            let still_valid = match kind {
                                PendingNotify::Complete => {
                                    state.light == "green"
                                        && state.notify_complete
                                        && !state.tasks.iter().any(|t| t.kind == "subagent")
                                }
                                PendingNotify::Attention => state.light == "red",
                            };
                            if still_valid {
                                send_notify(&app, kind);
                            }
                        }
                    }
                }
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

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(move |app| {
            write_pid_lock();

            let path = resolve_state_path();
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }

            start_state_watcher(app.handle().clone(), path, last_light.clone());

            if let Ok(exe) = std::env::current_exe() {
                if let Ok(store) = app.store(STORE_FILE) {
                    let _ = store.set(
                        EXE_PATH_KEY,
                        exe.to_string_lossy().to_string(),
                    );
                    let _ = store.save();
                }
            }

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
        .build(tauri::generate_context!())
        .expect("error while running tauri application");

    app.run(|_app_handle, event| {
        if let RunEvent::Exit = event {
            remove_pid_lock();
        }
    });
}