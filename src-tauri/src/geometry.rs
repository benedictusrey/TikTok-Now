//! Default-window geometry for TikTok-Now.
//!
//! v2.1.0: the default window is **1326 × 1032** — meaning the VISIBLE frame
//! (everything the user sees, title bar included) is exactly 1326 px wide and
//! exactly the height of the monitor's work area (screen minus taskbar —
//! 1032 px on the reference machine: 1920×1080 at 100 % DPI with a 48-px
//! taskbar), horizontally centered, flush with the screen top, and its bottom
//! edge resting exactly on the taskbar. (Width revised 1252 → 1326 by user
//! request, 2026-09-18; the fit mechanics are unchanged.)
//!
//! Why measure instead of hardcode 1032: the work-area height depends on
//! taskbar size/position and DPI (125 % scaling is common), so a hardcoded
//! height is only correct on one machine. The builder starts from
//! `DEFAULT_INNER_SIZE` (already correct on the reference machine from frame
//! one) and `fit_window_to_work_area` refines the placement using the REAL
//! measured work area — every other machine gets its own exact fit.

/// Default client (content) size in logical pixels.
/// Width 1326 is the visible frame width (the client spans the full visible
/// width on modern DWM windows — content reaches the window edges).
/// Height 1032 is the work-area height on the reference machine; the fit
/// routine below adjusts the client height for the title bar automatically.
pub const DEFAULT_INNER_SIZE: (f64, f64) = (1326.0, 1032.0);

/// Minimum window size in logical pixels (unchanged since v1.0.0).
pub const MIN_INNER_SIZE: (f64, f64) = (800.0, 600.0);

/// Resize the window so its **visible** frame (DWM extended frame bounds —
/// title bar included, invisible resize borders excluded) is exactly
/// `DEFAULT_INNER_SIZE.0` wide and exactly the monitor's work-area height
/// tall, horizontally centered in the work area, top flush with the work-area
/// top and the bottom edge on the taskbar.
///
/// Why not plain `set_size + center()`? Because `center()` centers the OUTER
/// window rect — which includes invisible resize borders — and the outer rect
/// is not what the user sees. This routine works in the coordinate space the
/// user actually sees:
///   * `DwmGetWindowAttribute(DWMWA_EXTENDED_FRAME_BOUNDS)` = the VISIBLE
///     window rect (excludes the invisible borders; includes the title bar),
///   * `GetWindowRect` − extended bounds = the hidden border thicknesses,
///   * so `outer = target_visible + hidden_borders` produces a visible frame
///     of exactly the requested size, wherever it is placed.
///
/// All measurements are physical pixels; DPI is handled by the OS conversion
/// APIs, so this is exact at 125 %/150 % scaling too. The width target is the
/// logical 1252 scaled by the window's real DPI factor. Sizes handed to
/// tao's `set_size` are CLIENT sizes (tao applies `AdjustWindowRect` itself —
/// verified against tao 0.35.3), so the routine computes the client height as
/// `work_area_height − caption_chrome` and positions the OUTER rect with the
/// invisible-border offsets.
///
/// Idempotency latch: the exact fit is applied once per process, the first
/// time the window is actually VISIBLE (DWM measurements on never-shown
/// windows are unreliable — the `--minimized` autostart path). `restore_window`
/// in tray.rs re-invokes this routine on every restore, so a hidden start
/// gets its exact fit the moment the user first shows the window; later
/// restores are no-ops and never fight the user's own resize.
static FITTED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Best effort: every OS call is infallible-guarded; if anything fails the
/// window simply keeps the builder's size/center (the pre-fit behavior).
#[cfg(windows)]
pub fn fit_window_to_work_area(window: &tauri::WebviewWindow) {
    use std::sync::atomic::Ordering;

    // Only ever fit while the window is visible (see latch docs) — and only
    // once. A second application would shrink a window the user resized.
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
        // 1. Ground truth: monitor work area (screen minus taskbar) in
        //    PHYSICAL pixels — tauri exposes exactly what we need.
        let Ok(Some(monitor)) = window.current_monitor() else {
            return;
        };
        let wa = monitor.work_area(); // PhysicalRect<i32, u32>
        // Copy the primitives out — `wa` borrows `monitor`, and the verify
        // thread below must not keep that borrow alive ('static bound).
        let wa_x = wa.position.x;
        let wa_y = wa.position.y;
        let wa_w = wa.size.width as i32;
        let wa_h = wa.size.height as i32;

        // The visible WIDTH target: 1252 logical px at the window's real DPI.
        let scale = window.scale_factor().unwrap_or(1.0);
        let visible_w = (DEFAULT_INNER_SIZE.0 * scale).round() as i32;
        let visible_x = wa_x + ((wa_w - visible_w) / 2).max(0);

        // 2. Measure the three rects that matter (all physical px):
        //      outer   (GetWindowRect)               — everything, incl. the
        //          invisible resize borders Win10/11 adds around the frame;
        //      visible (DWMWA_EXTENDED_FRAME_BOUNDS) — what the user SEES
        //          (title bar + visible edges, invisible borders excluded);
        //      client  (GetClientRect)               — the content area, which
        //          is what tao's `set_size` actually addresses (it runs
        //          AdjustWindowRect internally, so sizes given to it are CLIENT
        //          sizes — verified against tao 0.35.3's
        //          `set_inner_size_physical`).
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

        // Derived chrome metrics:
        //   border_left/top — invisible border between the outer rect and the
        //     visible frame (top ≈ 0 on Win10/11; saturating subtraction via
        //     plain ints keeps odd DWM configs from panicking — values are
        //     only ever used in additions that are clamped afterwards).
        //   top_chrome — caption height: visible height minus client height.
        let border_left = visible.left - outer.left;
        let border_top = visible.top - outer.top;
        let top_chrome = (visible.bottom - visible.top) - (client.bottom - client.top);

        // 3. Compute the CLIENT size that produces the target VISIBLE frame:
        //      visible width  = client width            (DWM windows: the
        //          content reaches the visible left/right edges),
        //      visible height = client height + top_chrome, and the target
        //          visible height IS the work-area height — so the bottom
        //          edge lands exactly on the taskbar.
        let client_w = visible_w.max(1);
        let client_h = (wa_h - top_chrome).max(1);

        // 4. Position: `set_position` addresses the OUTER rect, so offset by
        //    the invisible borders to place the VISIBLE frame.
        let outer_x = visible_x - border_left;
        let outer_y = wa_y - border_top;

        use tauri::PhysicalPosition;
        let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(
            client_w as u32,
            client_h as u32,
        )));
        let _ = window.set_position(PhysicalPosition::new(outer_x, outer_y));

        // 5. VERIFY the fit actually landed, and correct until it does
        //    (bounded). The set_size/set_position above are queued to the
        //    event loop — a correction computed before they are applied would
        //    double-apply and, worse, fire resizes WHILE the webview is
        //    crossing documents (splash→TikTok at ~350 ms), which shows up as
        //    black/white flashing (v2.1.0 audit round 4, F25). So: wait 450 ms
        //    FIRST (well past both the queued-op apply latency AND the splash
        //    redirect commit), measure the REAL frame, and only touch the
        //    window when a genuine deviation remains. In the normal case the
        //    queued ops land exactly on target and this loop performs ZERO
        //    additional resizes. Each pass re-derives the chrome metrics from
        //    fresh rects and re-applies BOTH size and position.
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

                // Deviation of the VISIBLE frame from its targets.
                let dw = visible_w - (vis.right - vis.left);
                let dh = wa_h - (vis.bottom - vis.top);

                if dw != 0 || dh != 0 {
                    // Size never landed (or landed stale): recompute chrome
                    // from the fresh rects and re-apply the client size.
                    let top_chrome =
                        (vis.bottom - vis.top) - (client.bottom - client.top);
                    let _ = w2.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(
                        visible_w.max(1) as u32,
                        (wa_h - top_chrome).max(1) as u32,
                    )));
                }

                let dx = visible_x - vis.left;
                let dy = wa_y - vis.top;
                if dx == 0 && dy == 0 && dw == 0 && dh == 0 {
                    return; // exact fit verified
                }
                let cur = w2.outer_position().unwrap_or_default();
                let _ = w2.set_position(PhysicalPosition::new(cur.x + dx, cur.y + dy));
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
        });
    }
}

/// Non-Windows builds keep the plain builder behavior (1252 × 1032, centered
/// in the work area by `center()`), which is already correct on macOS —
/// `NSWindow` frames exclude invisible borders and `visibleFrame`-based
/// centering respects the Dock.
#[cfg(not(windows))]
pub fn fit_window_to_work_area(_window: &tauri::WebviewWindow) {}
