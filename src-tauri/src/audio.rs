//! Windows audio-session control for TikTok-Now.
//!
//! Combines IG-Now's multi-endpoint `AudioSessionKeepAlive` + grouping GUID +
//! foreground focus unmute with X-Now's crash-safe `audio-muted.flag` marker
//! and `Mode::{Hidden, Visible, Release}` reconciliation:
//! - Brands and groups all process-tree sessions as `"TikTok-Now"` with the
//!   application icon across every active render endpoint (`EnumAudioEndpoints`).
//! - Silences the process tree while hidden/minimized and records
//!   `audio-muted.flag` on disk so a crash while hidden cannot leave the app
//!   permanently muted on the next launch.
//! - Unmutes on visible transition, on newly-created sessions, and while the
//!   window is in foreground focus; releases held mutes cleanly on exit.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use windows::core::{w, Interface, GUID, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, RPC_E_CHANGED_MODE};
#[cfg(debug_assertions)]
use windows::Win32::Media::Audio::Endpoints::IAudioMeterInformation;
use windows::Win32::Media::Audio::{
    eMultimedia, eRender, IAudioSessionControl, IAudioSessionControl2, IAudioSessionEnumerator,
    IAudioSessionManager2, IMMDeviceEnumerator, ISimpleAudioVolume, MMDeviceEnumerator,
    DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
    COINIT_MULTITHREADED,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};

const APP_DISPLAY_NAME: &str = "TikTok-Now";
const TIKTOK_NOW_AUDIO_GROUP: GUID = GUID::from_u128(0x7c8e1a42_3b5f_4d91_b2a0_81d4e6f93402);
const MARKER_FILE: &str = "audio-muted.flag";

/// What the watchdog wants this app's audio sessions to be doing.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Window hidden: mute every session in the tree, including new ones.
    Hidden,
    /// Window visible: clear mutes that should not persist.
    Visible,
    /// App exit: unmute what TikTok-Now muted and drop the marker.
    Release,
}

/// Decision for a single audio session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Mute,
    Unmute,
    Keep,
}

fn decide(mode: Mode, transition: bool, is_new: bool, muted: bool, ours: bool) -> Action {
    match mode {
        Mode::Hidden if !muted => Action::Mute,
        Mode::Hidden => Action::Keep,
        Mode::Visible if muted && (transition || is_new) => Action::Unmute,
        Mode::Visible => Action::Keep,
        Mode::Release if ours && muted => Action::Unmute,
        Mode::Release => Action::Keep,
    }
}

#[derive(Default)]
struct Book {
    muted_by_us: HashSet<String>,
    branded: HashSet<String>,
    marker: Option<PathBuf>,
    marker_on: bool,
    was_hidden: bool,
}

fn book() -> MutexGuard<'static, Book> {
    static BOOK: OnceLock<Mutex<Book>> = OnceLock::new();
    BOOK.get_or_init(|| Mutex::new(Book::default()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// Initialize the crash-safe mute marker in the app data directory.
pub fn init(data_dir: &Path) {
    let marker = data_dir.join(MARKER_FILE);
    let mut guard = book();
    guard.was_hidden = marker.exists();
    guard.marker_on = guard.was_hidden;
    guard.marker = Some(marker);
}

fn persist_marker(guard: &mut Book, hold: bool) {
    if hold == guard.marker_on {
        return;
    }
    if let Some(path) = &guard.marker {
        if hold {
            let _ = fs::write(path, b"muted");
        } else {
            let _ = fs::remove_file(path);
        }
    }
    guard.marker_on = hold;
}

fn exe_icon_utf16() -> Option<&'static [u16]> {
    static ICON_W: OnceLock<Option<Vec<u16>>> = OnceLock::new();
    ICON_W
        .get_or_init(|| {
            let exe = std::env::current_exe().ok()?;
            let wide: Vec<u16> = format!("{},0", exe.display())
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            Some(wide)
        })
        .as_deref()
}

/// PIDs of this process and all its descendants (covers `msedgewebview2.exe`
/// audio/renderer children owned by TikTok-Now).
fn process_tree() -> HashSet<u32> {
    let root = std::process::id();
    let mut tree = HashSet::with_capacity(16);
    tree.insert(root);

    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return tree;
        };
        let mut children: HashMap<u32, Vec<u32>> = HashMap::with_capacity(128);
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                children
                    .entry(entry.th32ParentProcessID)
                    .or_default()
                    .push(entry.th32ProcessID);
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);

        let mut queue = VecDeque::with_capacity(16);
        queue.push_back(root);
        while let Some(pid) = queue.pop_front() {
            if let Some(kids) = children.get(&pid) {
                for &kid in kids {
                    if tree.insert(kid) {
                        queue.push_back(kid);
                    }
                }
            }
        }
    }
    tree
}

unsafe fn with_com<F: FnOnce() -> bool>(f: F) -> bool {
    let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
    // Tauri UI thread is already STA (`RPC_E_CHANGED_MODE`), which still supports
    // Core Audio calls without calling `CoUninitialize`.
    if hr.is_err() && hr != RPC_E_CHANGED_MODE {
        return false;
    }
    let ok = f();
    if hr.is_ok() {
        CoUninitialize();
    }
    ok
}

unsafe fn brand_audio_session(session: &IAudioSessionControl) {
    let needs_label = match session.GetDisplayName() {
        Ok(pw) => {
            let current = pw.to_string().unwrap_or_default();
            CoTaskMemFree(Some(pw.as_ptr() as *const _));
            current != APP_DISPLAY_NAME
        }
        Err(_) => true,
    };
    let needs_group = session
        .GetGroupingParam()
        .map(|g| g != TIKTOK_NOW_AUDIO_GROUP)
        .unwrap_or(true);
    if needs_label {
        let _ = session.SetDisplayName(w!("TikTok-Now"), std::ptr::null());
        if let Some(icon_w) = exe_icon_utf16() {
            let _ = session.SetIconPath(PCWSTR(icon_w.as_ptr()), std::ptr::null());
        }
    }
    if needs_label || needs_group {
        let _ = session.SetGroupingParam(&TIKTOK_NOW_AUDIO_GROUP, std::ptr::null());
    }
}

unsafe fn instance_id(control: &IAudioSessionControl2) -> Option<String> {
    let raw = control.GetSessionInstanceIdentifier().ok()?;
    let id = raw.to_string().ok();
    CoTaskMemFree(Some(raw.0 as *const core::ffi::c_void));
    id
}

unsafe fn visit_active_render_managers<F>(enumerator: &IMMDeviceEnumerator, mut on_manager: F)
where
    F: FnMut(&IAudioSessionManager2),
{
    if let Ok(collection) = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) {
        if let Ok(dev_count) = collection.GetCount() {
            for idx in 0..dev_count {
                if let Ok(device) = collection.Item(idx) {
                    if let Ok(manager) = device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None)
                    {
                        on_manager(&manager);
                    }
                }
            }
            if dev_count > 0 {
                return;
            }
        }
    }
    if let Ok(device) = enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia) {
        if let Ok(manager) = device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) {
            on_manager(&manager);
        }
    }
}

unsafe fn enumerate_app_sessions<F>(mut visitor: F) -> bool
where
    F: FnMut(u32, Option<&str>, &IAudioSessionControl, &ISimpleAudioVolume),
{
    with_com(|| {
        let Ok(enumerator) =
            CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL)
        else {
            return false;
        };
        let tree = process_tree();
        visit_active_render_managers(&enumerator, |manager| {
            let Ok(sessions): Result<IAudioSessionEnumerator, _> = manager.GetSessionEnumerator()
            else {
                return;
            };
            let Ok(count) = sessions.GetCount() else {
                return;
            };
            for i in 0..count {
                let Ok(session) = sessions.GetSession(i) else {
                    continue;
                };
                let Ok(ctrl2) = session.cast::<IAudioSessionControl2>() else {
                    continue;
                };
                let Ok(pid) = ctrl2.GetProcessId() else {
                    continue;
                };
                if !tree.contains(&pid) {
                    continue;
                }
                let Ok(volume) = session.cast::<ISimpleAudioVolume>() else {
                    continue;
                };
                let id = instance_id(&ctrl2);
                visitor(pid, id.as_deref(), &session, &volume);
            }
        });
        true
    })
}

/// Returns active Windows audio render endpoint IDs so callers can detect
/// headphone/monitor hotplugs and refresh session branding.
pub fn active_render_endpoint_ids() -> Vec<String> {
    let mut ids = Vec::new();
    unsafe {
        let _ = with_com(|| {
            let Ok(enumerator) =
                CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL)
            else {
                return false;
            };
            if let Ok(collection) = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) {
                if let Ok(dev_count) = collection.GetCount() {
                    for idx in 0..dev_count {
                        if let Ok(device) = collection.Item(idx) {
                            if let Ok(pw) = device.GetId() {
                                let id = pw.to_string().unwrap_or_default();
                                CoTaskMemFree(Some(pw.as_ptr() as *const _));
                                if !id.is_empty() {
                                    ids.push(id);
                                }
                            }
                        }
                    }
                }
            }
            ids.sort_unstable();
            true
        });
    }
    ids
}

/// Keeps one grouped `"TikTok-Now"` Core Audio session handle open per active
/// render endpoint so Windows Volume Mixer recognizes `"TikTok-Now"` before
/// WebView2 starts its own media stream.
/// Releases COM interfaces BEFORE calling `CoUninitialize()` on drop.
pub struct AudioSessionKeepAlive {
    handles: Vec<IAudioSessionControl>,
    com_initialized: bool,
}

impl Drop for AudioSessionKeepAlive {
    fn drop(&mut self) {
        self.handles.clear();
        if self.com_initialized {
            unsafe {
                CoUninitialize();
            }
        }
    }
}

pub fn register_app_sessions_with_mute(start_muted: bool) -> AudioSessionKeepAlive {
    let mut handles = Vec::new();
    let mut com_initialized = false;
    unsafe {
        let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
        if hr.is_err() && hr != RPC_E_CHANGED_MODE {
            return AudioSessionKeepAlive {
                handles,
                com_initialized,
            };
        }
        com_initialized = hr.is_ok();
        if let Ok(enumerator) =
            CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL)
        {
            visit_active_render_managers(&enumerator, |manager| {
                if let Ok(ctrl) = manager.GetAudioSessionControl(Some(&TIKTOK_NOW_AUDIO_GROUP), 0)
                {
                    brand_audio_session(&ctrl);
                    if let Ok(vol) = ctrl.cast::<ISimpleAudioVolume>() {
                        let _ = vol.SetMute(start_muted, std::ptr::null());
                    }
                    handles.push(ctrl);
                }
            });
        }
    }
    AudioSessionKeepAlive {
        handles,
        com_initialized,
    }
}

pub fn register_app_sessions() -> AudioSessionKeepAlive {
    register_app_sessions_with_mute(false)
}

/// Reconcile this app's audio sessions with `mode` across all active render endpoints.
/// Returns `true` if at least one session mute state was changed.
pub fn sync(mode: Mode) -> bool {
    let mut changed = false;
    let mut seen = HashSet::new();
    let enumerated = unsafe {
        enumerate_app_sessions(|_pid, id_opt, session, volume| {
            let id = match id_opt {
                Some(s) => s.to_string(),
                None => return,
            };
            seen.insert(id.clone());

            let mut guard = book();
            let transition = mode == Mode::Visible && guard.was_hidden;
            let is_new = guard.branded.insert(id.clone());
            if is_new {
                brand_audio_session(session);
            } else {
                // Keep label/icon intact if Chromium resets display metadata.
                brand_audio_session(session);
            }

            let Ok(muted) = volume.GetMute() else {
                return;
            };
            let muted = muted.as_bool();
            let ours = guard.muted_by_us.contains(&id);

            match decide(mode, transition, is_new, muted, ours) {
                Action::Mute => {
                    if volume.SetMute(true, std::ptr::null()).is_ok() {
                        guard.muted_by_us.insert(id);
                        changed = true;
                    }
                }
                Action::Unmute => {
                    if volume.SetMute(false, std::ptr::null()).is_ok() {
                        guard.muted_by_us.remove(&id);
                        changed = true;
                    }
                }
                Action::Keep => {}
            }
        })
    };

    if enumerated {
        let mut guard = book();
        guard.branded.retain(|id| seen.contains(id));
        guard.muted_by_us.retain(|id| seen.contains(id));
        if mode == Mode::Release {
            guard.muted_by_us.clear();
        }
        match mode {
            Mode::Hidden => guard.was_hidden = true,
            Mode::Visible => guard.was_hidden = false,
            Mode::Release => {}
        }
        let hold = !guard.muted_by_us.is_empty();
        persist_marker(&mut guard, hold);
    }
    changed
}

/// Ensures every audio session in TikTok-Now's process tree stays labeled
/// `"TikTok-Now"` with the app icon/group and unmuted while foreground/visible.
pub fn ensure_foreground_audio_ready() -> bool {
    let mut unmuted_any = false;
    let enumerated = unsafe {
        enumerate_app_sessions(|_, id_opt, session, volume| {
            brand_audio_session(session);
            if matches!(volume.GetMute(), Ok(m) if m.as_bool())
                && volume.SetMute(false, std::ptr::null()).is_ok()
            {
                unmuted_any = true;
                if let Some(id) = id_opt {
                    book().muted_by_us.remove(id);
                }
            }
        })
    };
    if enumerated {
        let mut guard = book();
        guard.was_hidden = false;
        let hold = !guard.muted_by_us.is_empty();
        persist_marker(&mut guard, hold);
    }
    unmuted_any
}

/// Diagnostic: print every audio session owned by this app's process tree with
/// its MUTE flag, MASTER VOLUME, and peak meter. Debug builds only.
#[cfg(debug_assertions)]
pub fn report_audio_state() {
    let mut found = 0;
    unsafe {
        enumerate_app_sessions(|pid, _, session, volume| {
            brand_audio_session(session);
            let muted = volume
                .GetMute()
                .map(|m| if m.as_bool() { "yes" } else { "no" })
                .unwrap_or("?");
            let level = volume.GetMasterVolume().unwrap_or(-1.0);
            let peak = session
                .cast::<IAudioMeterInformation>()
                .ok()
                .and_then(|m| m.GetPeakValue().ok())
                .unwrap_or(-1.0);
            eprintln!(
                "[TikTok-Now] Audio state: pid={} muted={} volume={:.2} peak={:.4}",
                pid, muted, level, peak
            );
            found += 1;
        });
    }
    if found == 0 {
        eprintln!("[TikTok-Now] Audio state: no sessions in the process tree");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};

    #[test]
    fn process_tree_includes_current_pid_and_group_guid_is_non_zero() {
        let tree = process_tree();
        assert!(tree.contains(&std::process::id()));
        assert_ne!(TIKTOK_NOW_AUDIO_GROUP.data1, 0);
    }

    #[test]
    fn hiding_mutes_audible_sessions_and_keeps_muted_ones() {
        assert_eq!(
            decide(Mode::Hidden, false, false, false, false),
            Action::Mute
        );
        assert_eq!(
            decide(Mode::Hidden, false, true, false, false),
            Action::Mute
        );
        assert_eq!(
            decide(Mode::Hidden, false, false, true, true),
            Action::Keep
        );
    }

    #[test]
    fn first_visible_sync_after_hiding_clears_every_mute() {
        assert_eq!(
            decide(Mode::Visible, true, false, true, false),
            Action::Unmute
        );
        assert_eq!(
            decide(Mode::Visible, true, false, true, true),
            Action::Unmute
        );
        assert_eq!(
            decide(Mode::Visible, true, false, false, false),
            Action::Keep
        );
    }

    #[test]
    fn new_muted_session_is_cleared_while_visible_and_manual_mute_kept() {
        assert_eq!(
            decide(Mode::Visible, false, true, true, false),
            Action::Unmute
        );
        assert_eq!(
            decide(Mode::Visible, false, false, true, false),
            Action::Keep
        );
    }

    #[test]
    fn release_lifts_only_our_mutes() {
        assert_eq!(
            decide(Mode::Release, false, false, true, true),
            Action::Unmute
        );
        assert_eq!(
            decide(Mode::Release, false, false, true, false),
            Action::Keep
        );
        assert_eq!(
            decide(Mode::Release, false, false, false, true),
            Action::Keep
        );
    }

    #[test]
    fn audio_session_control_works_on_sta_thread_and_brands_tiktok_now() {
        unsafe {
            let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let _keepalive = register_app_sessions();
            let _muted_keepalive = register_app_sessions_with_mute(true);
            let _ = ensure_foreground_audio_ready();
            let _ = active_render_endpoint_ids();
            for _ in 0..100 {
                let tree = process_tree();
                assert!(tree.contains(&std::process::id()));
            }
            if hr.is_ok() {
                CoUninitialize();
            }
        }
    }

    #[test]
    fn audio_session_keepalive_safe_on_uninitialized_worker_thread() {
        std::thread::spawn(|| {
            let keepalive = register_app_sessions();
            drop(keepalive);
            let muted_keepalive = register_app_sessions_with_mute(true);
            drop(muted_keepalive);
        })
        .join()
        .expect("worker thread COM keepalive drop must not fail or panic");
    }

    #[test]
    fn marker_roundtrip_writes_and_removes_flag() {
        let mut dir = std::env::temp_dir();
        dir.push(format!("tiktoknow-audio-marker-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let flag = dir.join(MARKER_FILE);
        fs::write(&flag, b"muted").unwrap();

        let mut g = Book {
            was_hidden: flag.exists(),
            marker_on: flag.exists(),
            marker: Some(flag.clone()),
            ..Default::default()
        };
        assert!(g.was_hidden);
        assert!(g.marker_on);
        persist_marker(&mut g, false);
        assert!(!flag.exists());
        assert!(!g.marker_on);
        persist_marker(&mut g, true);
        assert!(flag.exists());
        assert!(g.marker_on);

        let _ = fs::remove_dir_all(&dir);
    }
}
