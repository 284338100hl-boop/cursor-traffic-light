//! Windows: keep the traffic-light window visible through "Show desktop" / Win+D.
//!
//! Strategy:
//! 1. Mark as tool window (WS_EX_TOOLWINDOW, skip taskbar) so shell treats it like a widget.
//! 2. Watch for minimize/hide and immediately restore without activating, then re-assert topmost.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Manager, WebviewWindow, WindowEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, GetWindowPlacement, IsIconic, IsWindowVisible, SetWindowLongPtrW,
    SetWindowPlacement, SetWindowPos, ShowWindow, GWL_EXSTYLE, HWND_TOPMOST, SWP_FRAMECHANGED,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SW_SHOWNOACTIVATE,
    WINDOWPLACEMENT, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
};

static GUARD_STARTED: AtomicBool = AtomicBool::new(false);

pub fn apply_tool_window(win: &WebviewWindow) {
    let _ = win.set_skip_taskbar(true);
    let Ok(hwnd) = win.hwnd() else {
        return;
    };
    unsafe {
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let new_ex = (ex & !(WS_EX_APPWINDOW.0 as isize)) | (WS_EX_TOOLWINDOW.0 as isize);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_ex);
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
        );
    }
}

fn needs_restore(win: &WebviewWindow) -> bool {
    let Ok(hwnd) = win.hwnd() else {
        return false;
    };
    unsafe { IsIconic(hwnd).as_bool() || !IsWindowVisible(hwnd).as_bool() }
}

pub fn restore_visible_topmost(win: &WebviewWindow, always_on_top: bool) {
    let Ok(hwnd) = win.hwnd() else {
        return;
    };
    unsafe {
        let mut wp = WINDOWPLACEMENT {
            length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
            ..Default::default()
        };
        if GetWindowPlacement(hwnd, &mut wp).is_ok() {
            wp.showCmd = SW_SHOWNOACTIVATE.0 as u32;
            let _ = SetWindowPlacement(hwnd, &wp);
        } else {
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
        if always_on_top {
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
    }
    if always_on_top {
        let _ = win.set_always_on_top(true);
    }
}

fn try_restore(app: &AppHandle) {
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    if !needs_restore(&win) {
        return;
    }
    // Only fight Show desktop when the user asked for always-on-top.
    if !crate::load_settings(app).always_on_top {
        return;
    }
    restore_visible_topmost(&win, true);
}

/// Install tool-window style plus minimize/hide recovery for Show desktop / Win+D.
pub fn install_show_desktop_guard(app: AppHandle, always_on_top: bool) {
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    apply_tool_window(&win);
    if always_on_top {
        restore_visible_topmost(&win, true);
    }

    let app_ev = app.clone();
    win.on_window_event(move |event| {
        if matches!(event, WindowEvent::Resized(_) | WindowEvent::Moved(_)) {
            try_restore(&app_ev);
        }
    });

    if GUARD_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    let app_poll = app.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(150));
        if app_poll.get_webview_window("main").is_none() {
            break;
        }
        try_restore(&app_poll);
    });
}
