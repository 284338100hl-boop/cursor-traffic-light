//! Windows: keep the traffic-light window visible and topmost through "Show desktop" / Win+D
//! and above aggressive activators (e.g. Electron/Cursor) without stealing focus.
//!
//! Strategy:
//! 1. Mark as tool window (WS_EX_TOOLWINDOW, skip taskbar).
//! 2. Restore if minimized/hidden.
//! 3. Re-assert HWND_TOPMOST whenever needed — not only when WS_EX_TOPMOST is cleared.
//!    Electron apps can rise above a topmost peer in the topmost z-band without clearing
//!    our style bit; SetWindowPos(HWND_TOPMOST) re-pins us to the top of that band.
//! 4. Hook EVENT_SYSTEM_FOREGROUND so activation of Cursor/others reasserts immediately.

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Manager, WebviewWindow, WindowEvent};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, GetWindowLongPtrW, GetWindowPlacement, IsIconic, IsWindowVisible,
    SetWindowLongPtrW, SetWindowPlacement, SetWindowPos, ShowWindow, TranslateMessage,
    EVENT_SYSTEM_FOREGROUND, GWL_EXSTYLE, HWND_TOPMOST, MSG, SWP_FRAMECHANGED, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SW_SHOWNOACTIVATE, WINDOWPLACEMENT,
    WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
};

static GUARD_STARTED: AtomicBool = AtomicBool::new(false);
static WANT_TOPMOST: AtomicBool = AtomicBool::new(true);
static MAIN_HWND: AtomicIsize = AtomicIsize::new(0);
static DELAYED_ASSERT_PENDING: AtomicBool = AtomicBool::new(false);

fn hwnd_of(win: &WebviewWindow) -> Option<HWND> {
    win.hwnd().ok()
}

fn hwnd_from_raw(raw: isize) -> Option<HWND> {
    if raw == 0 {
        return None;
    }
    Some(HWND(raw as *mut _))
}

fn store_main_hwnd(hwnd: HWND) {
    MAIN_HWND.store(hwnd.0 as isize, Ordering::SeqCst);
}

pub fn set_want_topmost(want: bool) {
    WANT_TOPMOST.store(want, Ordering::SeqCst);
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

fn assert_topmost_cached() {
    if !WANT_TOPMOST.load(Ordering::SeqCst) {
        return;
    }
    if let Some(hwnd) = hwnd_from_raw(MAIN_HWND.load(Ordering::SeqCst)) {
        assert_topmost(hwnd);
    }
}

/// Electron/Cursor often re-raises after activation; follow up without focus steal.
fn schedule_delayed_topmost_assert() {
    if DELAYED_ASSERT_PENDING.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(|| {
        for delay_ms in [60_u64, 180, 400] {
            std::thread::sleep(Duration::from_millis(delay_ms));
            assert_topmost_cached();
        }
        DELAYED_ASSERT_PENDING.store(false, Ordering::SeqCst);
    });
}

pub fn apply_tool_window(win: &WebviewWindow) {
    let _ = win.set_skip_taskbar(true);
    let Some(hwnd) = hwnd_of(win) else {
        return;
    };
    store_main_hwnd(hwnd);
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
    store_main_hwnd(hwnd);
    let did_show = needs_visibility_restore(hwnd);
    if did_show {
        restore_visibility_noactivate(hwnd);
    }
    if !always_on_top {
        return;
    }
    assert_topmost(hwnd);
    let _ = win.set_always_on_top(true);
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
    let always_on_top = crate::load_settings(app).always_on_top;
    set_want_topmost(always_on_top);
    if !always_on_top {
        return;
    }
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    let Some(hwnd) = hwnd_of(&win) else {
        return;
    };
    store_main_hwnd(hwnd);
    if needs_visibility_restore(hwnd) {
        restore_visible_topmost(&win, true);
        return;
    }
    // Always re-pin: style bit alone is not enough against Electron activation.
    assert_topmost(hwnd);
}

unsafe extern "system" fn on_foreground_changed(
    _hook: HWINEVENTHOOK,
    _event: u32,
    _hwnd: HWND,
    id_object: i32,
    id_child: i32,
    _id_event_thread: u32,
    _dwms_event_time: u32,
) {
    // OBJID_WINDOW == 0, CHILDID_SELF == 0
    if id_object != 0 || id_child != 0 {
        return;
    }
    assert_topmost_cached();
    schedule_delayed_topmost_assert();
}

fn start_foreground_hook() {
    std::thread::spawn(|| {
        unsafe {
            let flags = WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS;
            let hook = SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                None,
                Some(on_foreground_changed),
                0,
                0,
                flags,
            );
            if hook.0.is_null() {
                return;
            }
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    });
}

/// Install tool-window style plus minimize/hide/topmost recovery.
pub fn install_show_desktop_guard(app: AppHandle, always_on_top: bool) {
    set_want_topmost(always_on_top);
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

    start_foreground_hook();

    let app_poll = app.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(200));
        if app_poll.get_webview_window("main").is_none() {
            break;
        }
        try_enforce(&app_poll);
    });
}
