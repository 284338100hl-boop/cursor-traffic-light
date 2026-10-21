//! Windows: keep the traffic-light window visible and topmost through "Show desktop" / Win+D.
//!
//! Strategy:
//! 1. Mark as tool window (WS_EX_TOOLWINDOW, skip taskbar) so shell treats it like a widget.
//! 2. Watch for minimize/hide and immediately restore without activating.
//! 3. Re-assert HWND_TOPMOST whenever Show desktop (or anything else) demotes z-order —
//!    visibility alone is not enough; the shell often leaves the window shown but not topmost.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Manager, WebviewWindow, WindowEvent};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, GetWindowPlacement, IsIconic, IsWindowVisible, SetWindowLongPtrW,
    SetWindowPlacement, SetWindowPos, ShowWindow, GWL_EXSTYLE, HWND_TOPMOST, SWP_FRAMECHANGED,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SW_SHOWNOACTIVATE,
    WINDOWPLACEMENT, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
};

static GUARD_STARTED: AtomicBool = AtomicBool::new(false);

fn hwnd_of(win: &WebviewWindow) -> Option<HWND> {
    win.hwnd().ok()
}

fn is_topmost(hwnd: HWND) -> bool {
    unsafe {
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        ex & WS_EX_TOPMOST.0 != 0
    }
}

fn needs_visibility_restore(hwnd: HWND) -> bool {
    unsafe { IsIconic(hwnd).as_bool() || !IsWindowVisible(hwnd).as_bool() }
}

/// Force HWND_TOPMOST without activating or moving the window.
fn assert_topmost(hwnd: HWND) {
    unsafe {
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

pub fn apply_tool_window(win: &WebviewWindow) {
    let _ = win.set_skip_taskbar(true);
    let Some(hwnd) = hwnd_of(win) else {
        return;
    };
    unsafe {
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        // TOOLWINDOW + clear APPWINDOW; keep/restore TOPMOST via SetWindowPos below.
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

fn restore_visibility_noactivate(hwnd: HWND) {
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
    }
}

pub fn restore_visible_topmost(win: &WebviewWindow, always_on_top: bool) {
    let Some(hwnd) = hwnd_of(win) else {
        return;
    };
    let did_show = needs_visibility_restore(hwnd);
    if did_show {
        restore_visibility_noactivate(hwnd);
    }
    if !always_on_top {
        return;
    }
    assert_topmost(hwnd);
    let _ = win.set_always_on_top(true);
    // After a visibility restore, Show desktop may demote z-order a moment later.
    if did_show {
        let win2 = win.clone();
        std::thread::spawn(move || {
            for delay_ms in [40_u64, 120, 300] {
                std::thread::sleep(Duration::from_millis(delay_ms));
                if let Some(h) = hwnd_of(&win2) {
                    assert_topmost(h);
                }
            }
        });
    }
}

fn try_enforce(app: &AppHandle) {
    if !crate::load_settings(app).always_on_top {
        return;
    }
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    let Some(hwnd) = hwnd_of(&win) else {
        return;
    };
    let need_show = needs_visibility_restore(hwnd);
    let need_top = !is_topmost(hwnd);
    if need_show {
        restore_visible_topmost(&win, true);
    } else if need_top {
        // Stayed visible but lost TOPMOST (common after Show desktop).
        assert_topmost(hwnd);
        let _ = win.set_always_on_top(true);
    }
}

/// Install tool-window style plus minimize/hide/topmost recovery for Show desktop / Win+D.
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
        if matches!(
            event,
            WindowEvent::Resized(_) | WindowEvent::Moved(_) | WindowEvent::Focused(_)
        ) {
            try_enforce(&app_ev);
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
        try_enforce(&app_poll);
    });
}
