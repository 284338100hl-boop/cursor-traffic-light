use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

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

fn start_state_watcher(app: AppHandle, path: PathBuf) {
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let path = resolve_state_path();
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            start_state_watcher(app.handle().clone(), path);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_traffic_light_state,
            get_state_file_path
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
