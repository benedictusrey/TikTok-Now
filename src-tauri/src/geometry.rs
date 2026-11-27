//! Cross-platform work-area geometry for TikTok-Now.
//!
//! Main window target: **1326 × 1032** (clamped to the monitor's work area on
//! ALL platforms so smaller screens such as 1366×768, 1280×800, or 1600×900
//! never overflow off-screen or under the taskbar/Dock).
//!
//! On Windows, `fit_window_to_work_area` uses DWM extended frame bounds so the
//! visible frame lands cleanly inside the work area (ignoring Windows 10/11's
//! invisible resize borders). On macOS and Linux, `fit_window_to_work_area_on_monitor`
//! clamps and centers using Tauri's `Monitor::work_area()`, skipping
//! `set_position` on Wayland where compositors own window placement.

use std::sync::atomic::{AtomicBool, Ordering};

/// Default client (content) size in logical pixels for the main window.
pub const DEFAULT_INNER_SIZE: (f64, f64) = (1326.0, 1032.0);

/// Minimum main window size in logical pixels.
pub const MIN_INNER_SIZE: (f64, f64) = (800.0, 600.0);

/// Standalone Downloader window geometry (logical px).
pub const DOWNLOADER_WINDOW_WIDTH: f64 = 780.0;
pub const DOWNLOADER_WINDOW_HEIGHT: f64 = 860.0;
pub const DOWNLOADER_MIN_WIDTH: f64 = 580.0;
pub const DOWNLOADER_MIN_HEIGHT: f64 = 600.0;

static FITTED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorkAreaFrame {
    pub target_width: f64,
    pub target_height: f64,
    pub outer_x: i32,
    pub outer_y: i32,
}

#[allow(clippy::too_many_arguments)]
pub fn compute_centered_work_area_frame(
    work_x: i32,
    work_y: i32,
    work_width: u32,
    work_height: u32,
    overhead_width: u32,
    overhead_height: u32,
    scale: f64,
    default_width: f64,
    default_height: f64,
    min_width: f64,
    min_height: f64,
) -> WorkAreaFrame {
    let safe_scale = if scale > 0.0 { scale } else { 1.0 };
    let max_inner_width = ((work_width as f64 - overhead_width as f64) / safe_scale).max(1.0);
    let max_inner_height = ((work_height as f64 - overhead_height as f64) / safe_scale).max(1.0);
    let effective_min_w = min_width.min(max_inner_width);
    let effective_min_h = min_height.min(max_inner_height);
    let target_width = default_width.min(max_inner_width).max(effective_min_w);
    let target_height = default_height.min(max_inner_height).max(effective_min_h);
    let expected_outer_w = (target_width * safe_scale).round() + overhead_width as f64;
    let expected_outer_h = (target_height * safe_scale).round() + overhead_height as f64;
    let outer_x = work_x + (((work_width as f64 - expected_outer_w).max(0.0)) / 2.0).floor() as i32;
    let outer_y =
        work_y + (((work_height as f64 - expected_outer_h).max(0.0)) / 2.0).floor() as i32;
    WorkAreaFrame {
        target_width,
        target_height,
        outer_x,
        outer_y,
    }
}

pub fn is_wayland_session_from_env(
    wayland_display: Option<&str>,
    gdk_backend: Option<&str>,
) -> bool {
    if !cfg!(target_os = "linux") {
        return false;
    }
    if let Some(backend) = gdk_backend {
        if let Some(first) = backend.split(',').map(str::trim).find(|s| !s.is_empty()) {
            return first.eq_ignore_ascii_case("wayland");
        }
    }
    wayland_display.is_some_and(|v| !v.is_empty())
}

pub fn is_wayland_session() -> bool {
    let wayland = std::env::var("WAYLAND_DISPLAY").ok();
    let backend = std::env::var("GDK_BACKEND").ok();
    is_wayland_session_from_env(wayland.as_deref(), backend.as_deref())
}

/// Clamp and center a window to the target monitor's work area on all platforms.
pub fn fit_window_to_work_area_on_monitor(
    window: &tauri::WebviewWindow,
    anchor_monitor: Option<tauri::Monitor>,
    default_width: f64,
    default_height: f64,
    min_width: f64,
    min_height: f64,
) {
    let Ok(inner) = window.inner_size() else {
        return;
    };
    let Ok(outer) = window.outer_size() else {
        return;
    };
    let Some(monitor) = anchor_monitor
        .or_else(|| window.current_monitor().ok().flatten())
        .or_else(|| window.primary_monitor().ok().flatten())
    else {
        return;
    };
    let scale = monitor.scale_factor();
    let work = monitor.work_area();

    let overhead_width = outer.width.saturating_sub(inner.width);
    let overhead_height = outer.height.saturating_sub(inner.height);
    let frame = compute_centered_work_area_frame(
        work.position.x,
        work.position.y,
        work.size.width,
        work.size.height,
        overhead_width,
        overhead_height,
        scale,
        default_width,
        default_height,
        min_width,
        min_height,
    );

    let current_width = inner.width as f64 / scale;
    let current_height = inner.height as f64 / scale;
    let needs_resize = (current_width - frame.target_width).abs() > 0.5
        || (current_height - frame.target_height).abs() > 0.5;
    if needs_resize {
        let _ = window.set_size(tauri::LogicalSize::new(
            frame.target_width,
            frame.target_height,
        ));
    }

    if is_wayland_session() {
        return;
    }

    if !needs_resize {
        if let Ok(final_outer) = window.outer_size() {
            let x = work.position.x
                + (((work.size.width as f64 - final_outer.width as f64).max(0.0)) / 2.0).floor()
                    as i32;
            let y = work.position.y
                + (((work.size.height as f64 - final_outer.height as f64).max(0.0)) / 2.0).floor()
                    as i32;
            let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
            return;
        }
    }
    let _ = window.set_position(tauri::PhysicalPosition::new(frame.outer_x, frame.outer_y));
}

/// Resize and center the main window inside the monitor work area once per process.
#[cfg(windows)]
pub fn fit_window_to_work_area(window: &tauri::WebviewWindow) {
    if FITTED.load(Ordering::Acquire) || !window.is_visible().unwrap_or(false) {
        return;
    }
    FITTED.store(true, Ordering::Release);

    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Dwm::{
        DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetClientRect, GetWindowRect};

    let Ok(hwnd) = window.hwnd() else {
        return;
    };
    let hwnd = windows::Win32::Foundation::HWND(hwnd.0);

    unsafe {
        let Ok(Some(monitor)) = window.current_monitor() else {
            return;
        };
        let wa = monitor.work_area();
        let wa_x = wa.position.x;
        let wa_y = wa.position.y;
        let wa_w = wa.size.width as i32;
        let wa_h = wa.size.height as i32;

        let scale = window.scale_factor().unwrap_or(1.0);
        let desired_visible_w = (DEFAULT_INNER_SIZE.0 * scale).round() as i32;
        let desired_visible_h = (DEFAULT_INNER_SIZE.1 * scale).round() as i32;
        let visible_w = desired_visible_w.min(wa_w).max(1);
        let target_visible_h = desired_visible_h.min(wa_h).max(1);
        let visible_x = wa_x + ((wa_w - visible_w) / 2).max(0);
        let visible_y = wa_y + ((wa_h - target_visible_h) / 2).max(0);

        let mut outer = RECT::default();
        if GetWindowRect(hwnd, &mut outer).is_err() {
            return;
        }
        let mut visible = RECT::default();
        if DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut visible as *mut _ as *mut _,
            std::mem::size_of::<RECT>() as u32,
        )
        .is_err()
        {
            return;
        }
        let mut client = RECT::default();
        if GetClientRect(hwnd, &mut client).is_err() {
            return;
        }

        let border_left = visible.left - outer.left;
        let border_top = visible.top - outer.top;
        let top_chrome = (visible.bottom - visible.top) - (client.bottom - client.top);

        let client_w = visible_w.max(1);
        let client_h = (target_visible_h - top_chrome).max(1);

        let outer_x = visible_x - border_left;
        let outer_y = visible_y - border_top;

        use tauri::PhysicalPosition;
        let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(
            client_w as u32,
            client_h as u32,
        )));
        let _ = window.set_position(PhysicalPosition::new(outer_x, outer_y));

        let w2 = window.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(450));
            for _ in 0..8 {
                let Ok(hwnd2) = w2.hwnd() else { return };
                let hwnd2 = windows::Win32::Foundation::HWND(hwnd2.0);

                let mut outer = RECT::default();
                let mut vis = RECT::default();
                let mut client = RECT::default();
                if GetWindowRect(hwnd2, &mut outer).is_err()
                    || DwmGetWindowAttribute(
                        hwnd2,
                        DWMWA_EXTENDED_FRAME_BOUNDS,
                        &mut vis as *mut _ as *mut _,
                        std::mem::size_of::<RECT>() as u32,
                    )
                    .is_err()
                    || GetClientRect(hwnd2, &mut client).is_err()
                {
                    return;
                }

                let dw = visible_w - (vis.right - vis.left);
                let dh = target_visible_h - (vis.bottom - vis.top);

                if dw != 0 || dh != 0 {
                    let top_chrome =
                        (vis.bottom - vis.top) - (client.bottom - client.top);
                    let _ = w2.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(
                        visible_w.max(1) as u32,
                        (target_visible_h - top_chrome).max(1) as u32,
                    )));
                }

                let dx = visible_x - vis.left;
                let dy = visible_y - vis.top;
                if dx == 0 && dy == 0 && dw == 0 && dh == 0 {
                    return;
                }
                let cur = w2.outer_position().unwrap_or_default();
                let _ = w2.set_position(PhysicalPosition::new(cur.x + dx, cur.y + dy));
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
        });
    }
}

/// Non-Windows builds also clamp `1326 × 1032` to the monitor work area once
/// visible so 768p/800p/900p screens never overflow.
#[cfg(not(windows))]
pub fn fit_window_to_work_area(window: &tauri::WebviewWindow) {
    if FITTED.load(Ordering::Acquire) || !window.is_visible().unwrap_or(false) {
        return;
    }
    FITTED.store(true, Ordering::Release);
    fit_window_to_work_area_on_monitor(
        window,
        None,
        DEFAULT_INNER_SIZE.0,
        DEFAULT_INNER_SIZE.1,
        MIN_INNER_SIZE.0,
        MIN_INNER_SIZE.1,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_default_1326x1032_to_smaller_work_areas_and_centers_on_large_ones() {
        // 1366x768 laptop (48px taskbar -> 720px height): must clamp both width & height.
        let hd = compute_centered_work_area_frame(
            0,
            0,
            1366,
            720,
            16,
            39,
            1.0,
            DEFAULT_INNER_SIZE.0,
            DEFAULT_INNER_SIZE.1,
            MIN_INNER_SIZE.0,
            MIN_INNER_SIZE.1,
        );
        assert_eq!(hd.target_width, 1326.0);
        assert_eq!(hd.target_height, 681.0);
        assert_eq!(hd.outer_y, 0);

        // 1280x800 screen at 100% scale: width clamps to 1280 - 16 = 1264.
        let wxga = compute_centered_work_area_frame(
            0,
            0,
            1280,
            760,
            16,
            39,
            1.0,
            DEFAULT_INNER_SIZE.0,
            DEFAULT_INNER_SIZE.1,
            MIN_INNER_SIZE.0,
            MIN_INNER_SIZE.1,
        );
        assert_eq!(wxga.target_width, 1264.0);
        assert_eq!(wxga.target_height, 721.0);
        assert_eq!(wxga.outer_x, 0);
        assert_eq!(wxga.outer_y, 0);

        // 1920x1080 at 125% DPI scaling (1536x825 logical work area): must clamp cleanly.
        let fhd_125 = compute_centered_work_area_frame(
            0,
            0,
            1920,
            1032,
            20,
            49,
            1.25,
            DEFAULT_INNER_SIZE.0,
            DEFAULT_INNER_SIZE.1,
            MIN_INNER_SIZE.0,
            MIN_INNER_SIZE.1,
        );
        assert_eq!(fhd_125.target_width, 1326.0);
        assert!((fhd_125.target_height - (1032.0 - 49.0) / 1.25).abs() < 0.01);

        // 2560x1440 at 100% scale: keeps full 1326x1032 centered.
        let qhd = compute_centered_work_area_frame(
            0,
            0,
            2560,
            1392,
            16,
            39,
            1.0,
            DEFAULT_INNER_SIZE.0,
            DEFAULT_INNER_SIZE.1,
            MIN_INNER_SIZE.0,
            MIN_INNER_SIZE.1,
        );
        assert_eq!(qhd.target_width, 1326.0);
        assert_eq!(qhd.target_height, 1032.0);
        assert!(qhd.outer_x > 0 && qhd.outer_y > 0);
    }
}
