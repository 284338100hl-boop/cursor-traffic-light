use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, PhysicalSize, RunEvent,
    WindowEvent,
};
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
const TOAST_TEXT_KEY: &str = "toast_text";
const EXE_PATH_KEY: &str = "exe_path";
const DEFAULT_TOAST_TEXT: &str = "状态变化";
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
    pub toast_text: String,
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
            toast_text: DEFAULT_TOAST_TEXT.to_string(),
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

pub(crate) fn load_settings(app: &AppHandle) -> AppSettings {
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
        let toast_text = store
            .get(TOAST_TEXT_KEY)
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| DEFAULT_TOAST_TEXT.to_string());
        return AppSettings {
            idle_ms,
            notify_on_complete,
            notify_ntfy,
            ntfy_topic,
            always_on_top,
            show_hover_tooltip,
            auto_launch_with_cursor,
            toast_text,
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
        let _ = store.set(TOAST_TEXT_KEY, settings.toast_text.clone());
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

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum DockEdge {
    Left,
    Right,
    Top,
    Bottom,
}

const LIGHT_W: f64 = 140.0;
const LIGHT_H: f64 = 36.0;

fn light_physical(scale: f64) -> (u32, u32) {
    (
        (LIGHT_W * scale).round() as u32,
        (LIGHT_H * scale).round() as u32,
    )
}

struct DockInner {
    collapsed: bool,
    applying: bool,
    edge: DockEdge,
}

struct AppDock(Arc<Mutex<DockInner>>);
struct ToastGen(Mutex<u64>);

#[derive(Clone, Serialize)]
struct DockChanged {
    collapsed: bool,
    edge: Option<DockEdge>,
}

#[derive(Clone, Serialize)]
struct LightToastPayload {
    light: String,
    text: String,
    edge: DockEdge,
}

fn place_toast(toast: &tauri::WebviewWindow, x: i32, y: i32, w: i32, h: i32) {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{
            SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_SHOWWINDOW,
        };
        if let Ok(hwnd) = toast.hwnd() {
            let placed = unsafe {
                SetWindowPos(
                    hwnd,
                    Some(HWND_TOPMOST),
                    x,
                    y,
                    w,
                    h,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW,
                )
            };
            if placed.is_ok() {
                let _ = toast.show();
                return;
            }
        }
    }
    let _ = toast.set_size(PhysicalSize::new(w.max(0) as u32, h.max(0) as u32));
    let _ = toast.set_position(PhysicalPosition::new(x, y));
    let _ = toast.show();
}

fn peek_px(scale: f64) -> u32 {
    (10.0 * scale).round().max(8.0) as u32
}

fn monitor_bounds(
    win: &tauri::WebviewWindow,
) -> Option<(PhysicalPosition<i32>, PhysicalSize<u32>, f64)> {
    let monitor = win.current_monitor().ok().flatten()?;
    Some((*monitor.position(), *monitor.size(), monitor.scale_factor()))
}

fn nearest_edge(
    pos: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
    origin: PhysicalPosition<i32>,
    msize: PhysicalSize<u32>,
    threshold: i32,
) -> Option<DockEdge> {
    let right = origin.x + msize.width as i32;
    let bottom = origin.y + msize.height as i32;
    let gaps = [
        (DockEdge::Left, pos.x - origin.x),
        (DockEdge::Right, right - (pos.x + size.width as i32)),
        (DockEdge::Top, pos.y - origin.y),
        (DockEdge::Bottom, bottom - (pos.y + size.height as i32)),
    ];
    let mut best: Option<(DockEdge, i32)> = None;
    for (edge, gap) in gaps {
        if gap > threshold {
            continue;
        }
        match best {
            None => best = Some((edge, gap)),
            Some((_, g)) if gap < g => best = Some((edge, gap)),
            _ => {}
        }
    }
    best.map(|(e, _)| e)
}

fn clamp_axis(v: i32, span: u32, origin: i32, limit: u32) -> i32 {
    let max = origin + limit as i32 - span as i32;
    if max <= origin {
        return origin;
    }
    v.clamp(origin, max)
}

fn apply_peek(
    win: &tauri::WebviewWindow,
    edge: DockEdge,
    pos: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
    origin: PhysicalPosition<i32>,
    msize: PhysicalSize<u32>,
    scale: f64,
) {
    let peek = peek_px(scale);
    let (x, y, w, h) = match edge {
        DockEdge::Left => (
            origin.x,
            clamp_axis(pos.y, size.height, origin.y, msize.height),
            peek,
            size.height,
        ),
        DockEdge::Right => (
            origin.x + msize.width as i32 - peek as i32,
            clamp_axis(pos.y, size.height, origin.y, msize.height),
            peek,
            size.height,
        ),
        DockEdge::Top => (
            clamp_axis(pos.x, size.width, origin.x, msize.width),
            origin.y,
            size.width,
            peek,
        ),
        DockEdge::Bottom => (
            clamp_axis(pos.x, size.width, origin.x, msize.width),
            origin.y + msize.height as i32 - peek as i32,
            size.width,
            peek,
        ),
    };
    let _ = win.set_min_size(Some(LogicalSize::new(8.0, 8.0)));
    let _ = win.set_resizable(false);
    let _ = win.set_size(PhysicalSize::new(w, h));
    let _ = win.set_position(PhysicalPosition::new(x, y));
}

fn expand_at(
    win: &tauri::WebviewWindow,
    edge: DockEdge,
    peek_pos: PhysicalPosition<i32>,
    origin: PhysicalPosition<i32>,
    msize: PhysicalSize<u32>,
    scale: f64,
) {
    let (rest_w, rest_h) = light_physical(scale);
    let (x, y) = match edge {
        DockEdge::Left => (
            origin.x,
            clamp_axis(peek_pos.y, rest_h, origin.y, msize.height),
        ),
        DockEdge::Right => (
            origin.x + msize.width as i32 - rest_w as i32,
            clamp_axis(peek_pos.y, rest_h, origin.y, msize.height),
        ),
        DockEdge::Top => (
            clamp_axis(peek_pos.x, rest_w, origin.x, msize.width),
            origin.y,
        ),
        DockEdge::Bottom => (
            clamp_axis(peek_pos.x, rest_w, origin.x, msize.width),
            origin.y + msize.height as i32 - rest_h as i32,
        ),
    };
    let _ = win.set_min_size(Some(LogicalSize::new(LIGHT_W, LIGHT_H)));
    let _ = win.set_resizable(false);
    let _ = win.set_size(LogicalSize::new(LIGHT_W, LIGHT_H));
    let _ = win.set_position(PhysicalPosition::new(x, y));
}

fn expand_at_drop(
    win: &tauri::WebviewWindow,
    pos: PhysicalPosition<i32>,
    origin: PhysicalPosition<i32>,
    msize: PhysicalSize<u32>,
    scale: f64,
) {
    let (rest_w, rest_h) = light_physical(scale);
    let x = clamp_axis(pos.x, rest_w, origin.x, msize.width);
    let y = clamp_axis(pos.y, rest_h, origin.y, msize.height);
    let _ = win.set_min_size(Some(LogicalSize::new(LIGHT_W, LIGHT_H)));
    let _ = win.set_resizable(false);
    let _ = win.set_size(LogicalSize::new(LIGHT_W, LIGHT_H));
    let _ = win.set_position(PhysicalPosition::new(x, y));
}

fn schedule_clear_applying(dock: &AppDock) {
    let inner = Arc::clone(&dock.0);
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(400));
        if let Ok(mut g) = inner.lock() {
            g.applying = false;
        }
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

#[tauri::command]
fn hide_context_menu(app: AppHandle) {
    if let Some(window) = app.get_webview_window("menu") {
        let _ = window.hide();
    }
}

#[tauri::command]
fn show_context_menu(app: AppHandle, client_x: f64, client_y: f64) {
    let Some(main) = app.get_webview_window("main") else {
        return;
    };
    let Some(menu) = app.get_webview_window("menu") else {
        return;
    };
    let Ok(pos) = main.outer_position() else {
        return;
    };
    let Ok(scale) = main.scale_factor() else {
        return;
    };
    let x = pos.x + (client_x * scale).round() as i32;
    let y = pos.y + (client_y * scale).round() as i32;
    let _ = menu.set_position(PhysicalPosition::new(x, y));
    let _ = menu.show();
    let _ = menu.set_focus();
}

#[tauri::command]
fn exit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn hide_light_toast(app: AppHandle) {
    if let Some(window) = app.get_webview_window("toast") {
        let _ = window.hide();
    }
}

#[tauri::command]
fn show_light_toast(
    app: AppHandle,
    dock: tauri::State<AppDock>,
    gen: tauri::State<ToastGen>,
    light: String,
) {
    let g = dock.0.lock().unwrap();
    if !g.collapsed {
        return;
    }
    let edge = g.edge;
    drop(g);
    let Some(main) = app.get_webview_window("main") else {
        return;
    };
    let Some(toast) = app.get_webview_window("toast") else {
        return;
    };
    let Ok(peek_pos) = main.outer_position() else {
        return;
    };
    let Ok(peek_size) = main.outer_size() else {
        return;
    };
    let Some((origin, msize, scale)) = monitor_bounds(&main) else {
        return;
    };
    let tw = (220.0 * scale).round() as i32;
    let th = (64.0 * scale).round() as i32;
    let (x, y) = match edge {
        DockEdge::Left => (
            peek_pos.x + peek_size.width as i32,
            peek_pos.y + peek_size.height as i32 / 2 - th / 2,
        ),
        DockEdge::Right => (
            peek_pos.x - tw,
            peek_pos.y + peek_size.height as i32 / 2 - th / 2,
        ),
        DockEdge::Top => (
            peek_pos.x + peek_size.width as i32 / 2 - tw / 2,
            peek_pos.y + peek_size.height as i32,
        ),
        DockEdge::Bottom => (
            peek_pos.x + peek_size.width as i32 / 2 - tw / 2,
            peek_pos.y - th,
        ),
    };
    let x = clamp_axis(x, tw as u32, origin.x, msize.width);
    let y = clamp_axis(y, th as u32, origin.y, msize.height);
    let text = load_settings(&app).toast_text;
    let _ = app.emit(
        "light-toast",
        LightToastPayload { light, text, edge },
    );
    place_toast(&toast, x, y, tw, th);
    let mut seq = gen.0.lock().unwrap();
    *seq += 1;
    let my = *seq;
    drop(seq);
    let app2 = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(2200));
        let gen = app2.state::<ToastGen>();
        let latest = gen.0.lock().unwrap();
        if *latest != my {
            return;
        }
        drop(latest);
        if let Some(window) = app2.get_webview_window("toast") {
            let _ = window.hide();
        }
    });
}

#[tauri::command]
fn expand_dock(app: AppHandle, dock: tauri::State<AppDock>) {
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    let Some((origin, msize, scale)) = monitor_bounds(&win) else {
        return;
    };
    let Ok(peek_pos) = win.outer_position() else {
        return;
    };
    let mut g = dock.0.lock().unwrap();
    if !g.collapsed {
        return;
    }
    g.collapsed = false;
    g.applying = true;
    let edge = g.edge;
    drop(g);
    hide_light_toast(app.clone());
    expand_at(&win, edge, peek_pos, origin, msize, scale);
    let _ = app.emit(
        "dock-changed",
        DockChanged {
            collapsed: false,
            edge: None,
        },
    );
    schedule_clear_applying(&*dock);
}

#[tauri::command]
fn dock_drag_end(app: AppHandle, dock: tauri::State<AppDock>) {
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    let mut g = dock.0.lock().unwrap();
    if g.applying {
        return;
    }
    let Ok(pos) = win.outer_position() else {
        return;
    };
    let Ok(size) = win.outer_size() else {
        return;
    };
    let Some((origin, msize, scale)) = monitor_bounds(&win) else {
        return;
    };
    let threshold = (20.0 * scale).round() as i32;
    let edge = nearest_edge(pos, size, origin, msize, threshold);
    match edge {
        Some(e) => {
            g.collapsed = true;
            g.edge = e;
            g.applying = true;
            drop(g);
            let (light_w, light_h) = light_physical(scale);
            let along = match e {
                DockEdge::Left | DockEdge::Right => PhysicalSize::new(size.width, light_h),
                DockEdge::Top | DockEdge::Bottom => PhysicalSize::new(light_w, size.height),
            };
            apply_peek(&win, e, pos, along, origin, msize, scale);
            let _ = app.emit(
                "dock-changed",
                DockChanged {
                    collapsed: true,
                    edge: Some(e),
                },
            );
            schedule_clear_applying(&*dock);
        }
        None => {
            if !g.collapsed {
                return;
            }
            g.collapsed = false;
            g.applying = true;
            drop(g);
            hide_light_toast(app.clone());
            expand_at_drop(&win, pos, origin, msize, scale);
            let _ = app.emit(
                "dock-changed",
                DockChanged {
                    collapsed: false,
                    edge: None,
                },
            );
            schedule_clear_applying(&*dock);
        }
    }
}

#[cfg(windows)]
mod win_persist;

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
        .manage(AppDock(Arc::new(Mutex::new(DockInner {
            collapsed: false,
            applying: false,
            edge: DockEdge::Left,
        }))))
        .manage(ToastGen(Mutex::new(0)))
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
                let _ = window.set_resizable(false);
                let _ = window.set_size(LogicalSize::new(LIGHT_W, LIGHT_H));
                // 主窗口不占任务栏；退出请用右键菜单「退出」
                let _ = window.set_skip_taskbar(true);
            }

            #[cfg(windows)]
            win_persist::install_show_desktop_guard(app.handle().clone(), settings.always_on_top);

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

            if let Some(window) = app.get_webview_window("menu") {
                let window_handle = window.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::Focused(false) = event {
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
            show_settings_window,
            hide_context_menu,
            show_context_menu,
            exit_app,
            hide_light_toast,
            show_light_toast,
            expand_dock,
            dock_drag_end
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application");

    app.run(|_app_handle, event| {
        if let RunEvent::Exit = event {
            remove_pid_lock();
        }
    });
}