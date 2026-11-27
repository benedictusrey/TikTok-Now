// Copyright (c) 2026 Benedictus Reynaldo Hartanto (@benedictusrey). All rights reserved.
// TikTok-Now — High-performance desktop client for TikTok (https://github.com/benedictusrey/TikTok-Now)
// Licensed under the MIT License.

use std::{
    collections::{HashMap, HashSet},
    env,
    ffi::OsStr,
    fs,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, Url};
use tokio::sync::oneshot;

static TEMP_SEQ: AtomicU64 = AtomicU64::new(1);

const DEFAULT_UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36";

#[allow(dead_code)]
pub fn default_user_agent() -> &'static str {
    DEFAULT_UA
}

/// Hide child console windows on Windows when spawning `curl.exe` or `ffmpeg.exe`.
pub fn apply_no_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = cmd;
}

/// Resolve the OS-native `curl` binary, avoiding third-party shims on Windows.
pub fn system_curl_bin() -> String {
    #[cfg(windows)]
    {
        let root = env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
        format!("{root}\\System32\\curl.exe")
    }
    #[cfg(not(windows))]
    {
        let sys_curl = Path::new("/usr/bin/curl");
        if sys_curl.is_file() {
            return sys_curl.to_string_lossy().into_owned();
        }
        "curl".to_string()
    }
}

/// Return true if the base filename stem is a Windows reserved DOS device name.
pub fn is_windows_reserved_stem(raw: &str) -> bool {
    let base = raw.split('.').next().unwrap_or(raw).trim();
    matches!(
        base.to_ascii_uppercase().as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MediaFormat {
    pub id: String,
    pub label: String,
    pub width: u32,
    pub height: u32,
    pub bandwidth: u64,
    #[serde(default)]
    pub codecs: String,
    pub ext: String,
    pub requires_mux: bool,
    pub video_url: String,
    pub audio_url: Option<String>,
    #[serde(default)]
    pub fallback_urls: Vec<String>,
}

#[allow(dead_code)]
pub type VideoFormat = MediaFormat;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MediaItem {
    pub index: usize,
    pub kind: String,
    pub thumbnail_url: Option<String>,
    pub formats: Vec<MediaFormat>,
}

#[allow(dead_code)]
pub type MediaItemSpec = MediaItem;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InspectedPost {
    pub video_id: String,
    pub username: String,
    pub display_name: String,
    pub post_url: String,
    pub created_at: String,
    pub caption: String,
    #[serde(default)]
    pub music_title: Option<String>,
    #[serde(default)]
    pub music_author: Option<String>,
    #[serde(default)]
    pub music_url: Option<String>,
    pub ffmpeg_available: bool,
    pub items: Vec<MediaItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadOutcome {
    pub saved_paths: Vec<String>,
    pub caption_path: Option<String>,
    pub metadata_path: Option<String>,
    pub subfolder: Option<String>,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DownloaderContext {
    pub url: String,
    pub payload_json: Option<String>,
    #[serde(default)]
    pub item_index: Option<usize>,
    #[serde(default)]
    pub item_hint: Option<String>,
    #[serde(default)]
    pub theme: Option<String>,
}

pub type PendingDownloadOpen = DownloaderContext;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashTrack {
    pub url: String,
    pub width: u32,
    pub height: u32,
    pub bandwidth: u64,
    pub codecs: String,
    pub is_audio: bool,
}

#[derive(Default)]
pub struct DownloaderProbeState {
    pub pending_open: Mutex<Option<DownloaderContext>>,
    pub pending_probes: Mutex<HashMap<String, oneshot::Sender<String>>>,
}

impl DownloaderProbeState {
    pub fn set_pending_open(&self, open: Option<DownloaderContext>) {
        let mut slot = self.pending_open.lock().unwrap_or_else(|e| e.into_inner());
        *slot = open;
    }

    pub fn take_pending_open(&self) -> Option<DownloaderContext> {
        self.pending_open
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
    }

    pub fn register_probe(&self, request_id: String, tx: oneshot::Sender<String>) {
        let mut map = self
            .pending_probes
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        map.insert(request_id, tx);
    }

    pub fn resolve_probe(&self, request_id: &str, payload: String) -> bool {
        let sender = self
            .pending_probes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(request_id);
        match sender {
            Some(tx) => tx.send(payload).is_ok(),
            None => false,
        }
    }

    pub fn cancel_probe(&self, request_id: &str) {
        self.pending_probes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(request_id);
    }
}

/// RAII guard that removes a pending live-probe entry if the caller times out or drops early.
pub struct ProbeCancelGuard<'a> {
    probe_state: &'a DownloaderProbeState,
    request_id: String,
}

impl<'a> ProbeCancelGuard<'a> {
    pub fn new(probe_state: &'a DownloaderProbeState, request_id: String) -> Self {
        Self {
            probe_state,
            request_id,
        }
    }
}

impl Drop for ProbeCancelGuard<'_> {
    fn drop(&mut self) {
        self.probe_state.cancel_probe(&self.request_id);
    }
}

pub fn is_safe_http_url(url: &str) -> bool {
    let trimmed = url.trim();
    (trimmed.starts_with("https://") || trimmed.starts_with("http://"))
        && !trimmed.contains(['\r', '\n', '\0'])
}

/// Returns true only for first-party TikTok web/API hosts that may receive TikTok session cookies.
pub fn is_tiktok_app_host(url: &str) -> bool {
    Url::parse(url.trim())
        .ok()
        .filter(|parsed| parsed.username().is_empty() && parsed.password().is_none())
        .and_then(|parsed| parsed.host_str().map(|h| h.to_ascii_lowercase()))
        .is_some_and(|host| host == "tiktok.com" || host.ends_with(".tiktok.com"))
}

pub fn is_tiktok_share_url(url: &str) -> bool {
    Url::parse(url.trim())
        .ok()
        .filter(|parsed| parsed.username().is_empty() && parsed.password().is_none())
        .and_then(|parsed| parsed.host_str().map(|h| h.to_ascii_lowercase()))
        .is_some_and(|host| {
            host == "tiktok.com"
                || host.ends_with(".tiktok.com")
                || host == "tiktokv.com"
                || host.ends_with(".tiktokv.com")
        })
}

fn is_tiktok_cdn_host(host: &str) -> bool {
    const EXACT_SUFFIXES: &[&str] = &[
        "tiktok.com",
        "tiktokcdn.com",
        "tiktokcdn-us.com",
        "tiktokcdn-eu.com",
        "tiktokcdn-in.com",
        "tiktokv.com",
        "tiktokv.us",
        "tiktokv.eu",
        "muscdn.com",
        "byteoversea.com",
        "ibytedtos.com",
        "ibyteimg.com",
        "byteimg.com",
        "ttwstatic.com",
        "pstatp.com",
        "snssdk.com",
        "amemv.com",
    ];
    if EXACT_SUFFIXES
        .iter()
        .any(|d| host == *d || host.ends_with(&format!(".{d}")))
    {
        return true;
    }
    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() >= 2 {
        let sld = labels[labels.len() - 2];
        let tld = labels[labels.len() - 1];
        if (sld == "tiktokcdn" || sld.starts_with("tiktokcdn-")) && tld == "com" {
            return true;
        }
    }
    false
}

pub fn is_tiktok_media_url(url: &str) -> bool {
    let trimmed = url.trim();
    if !trimmed.starts_with("https://") || trimmed.contains(['\r', '\n', '\0']) {
        return false;
    }
    Url::parse(trimmed)
        .ok()
        .filter(|parsed| parsed.username().is_empty() && parsed.password().is_none())
        .and_then(|parsed| parsed.host_str().map(|h| h.to_ascii_lowercase()))
        .is_some_and(|host| is_tiktok_cdn_host(&host))
}

pub fn unescape_url(raw: &str) -> String {
    raw.trim()
        .replace("\\u0026", "&")
        .replace("\\u003A", ":")
        .replace("\\u003a", ":")
        .replace("\\u002F", "/")
        .replace("\\u002f", "/")
        .replace("\\u003F", "?")
        .replace("\\u003f", "?")
        .replace("\\u003D", "=")
        .replace("\\u003d", "=")
        .replace("\\/", "/")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
}

/// Strip MSE/DASH byte-range query parameters so downloads fetch the complete stream.
pub fn strip_media_range_params(raw: &str) -> String {
    let decoded = unescape_url(raw);
    if !is_safe_http_url(&decoded) {
        return String::new();
    }
    let Some((base, query)) = decoded.split_once('?') else {
        return decoded;
    };
    let (query, fragment) = match query.split_once('#') {
        Some((q, f)) => (q, Some(f)),
        None => (query, None),
    };
    let kept: Vec<&str> = query
        .split('&')
        .filter(|pair| {
            let key = pair.split('=').next().unwrap_or("");
            !matches!(
                key.to_ascii_lowercase().as_str(),
                "bytestart" | "byteend" | "range" | "segment" | "chunk" | "part"
            )
        })
        .collect();
    let mut rebuilt = base.to_string();
    if !kept.is_empty() {
        rebuilt.push('?');
        rebuilt.push_str(&kept.join("&"));
    }
    if let Some(frag) = fragment {
        rebuilt.push('#');
        rebuilt.push_str(frag);
    }
    rebuilt
}

fn sanitize_media_url(raw: &str) -> Option<String> {
    let cleaned = strip_media_range_params(raw);
    if cleaned.is_empty() || !is_tiktok_media_url(&cleaned) {
        None
    } else {
        Some(cleaned)
    }
}

fn sanitize_header_value(raw: &str) -> String {
    raw.chars()
        .filter(|&c| c != '\r' && c != '\n' && c != '\0')
        .collect()
}

pub fn is_tiktok_shortlink(input: &str) -> bool {
    let Ok(parsed) = Url::parse(input.trim()) else {
        return false;
    };
    let Some(host) = parsed.host_str().map(|h| h.to_ascii_lowercase()) else {
        return false;
    };
    host == "vm.tiktok.com"
        || host == "vt.tiktok.com"
        || ((host == "tiktok.com" || host.ends_with(".tiktok.com"))
            && parsed.path().starts_with("/t/"))
}

pub fn video_id_from_url(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.chars().all(|c| c.is_ascii_digit()) && (5..=24).contains(&trimmed.len()) {
        return Some(trimmed.to_string());
    }
    if (trimmed.contains("://") || trimmed.to_ascii_lowercase().starts_with("http"))
        && !is_tiktok_share_url(trimmed)
    {
        return None;
    }
    for marker in ["/video/", "/photo/", "/embed/v2/", "/v/"] {
        if let Some(pos) = trimmed.find(marker) {
            let rest = &trimmed[pos + marker.len()..];
            let seg = rest
                .split(['/', '?', '#', '&', '.'])
                .next()
                .unwrap_or("")
                .trim();
            if !seg.is_empty() && seg.chars().all(|c| c.is_ascii_digit()) {
                return Some(seg.to_string());
            }
        }
    }
    if let Ok(parsed) = Url::parse(trimmed) {
        for (k, v) in parsed.query_pairs() {
            if matches!(k.as_ref(), "item_id" | "aweme_id" | "video_id") {
                let id = v.trim();
                if !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()) {
                    return Some(id.to_string());
                }
            }
        }
    }
    None
}

pub fn username_from_url(input: &str) -> Option<String> {
    let parsed = Url::parse(input.trim()).ok()?;
    if !is_tiktok_share_url(input) {
        return None;
    }
    let first = parsed.path_segments()?.next()?.trim();
    if let Some(user) = first.strip_prefix('@') {
        let clean = user.trim();
        if !clean.is_empty() && clean != "i" {
            return Some(clean.to_string());
        }
    }
    None
}

pub fn canonical_post_url(username: &str, video_id: &str, is_photo: bool) -> String {
    let kind_seg = if is_photo { "photo" } else { "video" };
    if !video_id.trim().is_empty() {
        let user = username.trim().trim_start_matches('@');
        if !user.is_empty() {
            format!("https://www.tiktok.com/@{user}/{kind_seg}/{}", video_id.trim())
        } else {
            format!("https://www.tiktok.com/@i/{kind_seg}/{}", video_id.trim())
        }
    } else {
        "https://www.tiktok.com/".to_string()
    }
}

fn read_xml_attr(tag: &str, name: &str) -> Option<String> {
    let lower_tag = tag.to_ascii_lowercase();
    let needle = format!("{}=", name.to_ascii_lowercase());
    let mut offset = 0;
    while let Some(pos) = lower_tag[offset..].find(&needle) {
        let abs = offset + pos;
        let prev_ok = abs == 0
            || lower_tag.as_bytes()[abs - 1].is_ascii_whitespace()
            || lower_tag.as_bytes()[abs - 1] == b'<';
        let after = &tag[abs + needle.len()..];
        offset = abs + needle.len();
        if !prev_ok {
            continue;
        }
        let quote = after.chars().next()?;
        if quote != '"' && quote != '\'' {
            continue;
        }
        let value = &after[quote.len_utf8()..];
        if let Some(end) = value.find(quote) {
            return Some(unescape_url(&value[..end]));
        }
    }
    None
}

fn tag_blocks<'a>(source: &'a str, tag: &str) -> Vec<&'a str> {
    let open_prefix = format!("<{}", tag.to_ascii_lowercase());
    let close_tag = format!("</{}>", tag.to_ascii_lowercase());
    let lower = source.to_ascii_lowercase();
    let mut blocks = Vec::new();
    let mut pos = 0;
    while let Some(start) = lower[pos..].find(&open_prefix) {
        let abs_start = pos + start;
        let next_char = lower.as_bytes().get(abs_start + open_prefix.len()).copied();
        if !matches!(next_char, Some(b'>' | b' ' | b'\t' | b'\r' | b'\n')) {
            pos = abs_start + open_prefix.len();
            continue;
        }
        if let Some(end_rel) = lower[abs_start..].find(&close_tag) {
            let abs_end = abs_start + end_rel + close_tag.len();
            blocks.push(&source[abs_start..abs_end]);
            pos = abs_end;
        } else {
            break;
        }
    }
    blocks
}

fn extract_tag_inner(block: &str, tag: &str) -> Option<String> {
    let lower = block.to_ascii_lowercase();
    let open = format!("<{}", tag.to_ascii_lowercase());
    let close = format!("</{}>", tag.to_ascii_lowercase());
    let start = lower.find(&open)?;
    let gt = lower[start..].find('>')? + start + 1;
    let end = lower[gt..].find(&close)? + gt;
    Some(block[gt..end].trim().to_string())
}

pub fn video_codec_rank(codecs: &str) -> u8 {
    let c = codecs.trim().to_ascii_lowercase();
    if c.is_empty() {
        return 2;
    }
    if c.contains("avc1") || c.contains("avc3") || c.contains("h264") {
        4
    } else if c.contains("hev1")
        || c.contains("hvc1")
        || c.contains("hevc")
        || c.contains("h265")
        || c.contains("bytevc1")
    {
        3
    } else if c.contains("vp09") || c.contains("vp9") || c.contains("vp8") {
        1
    } else if c.contains("av01") || c.contains("av1") {
        0
    } else {
        2
    }
}

pub fn audio_codec_rank(codecs: &str) -> u8 {
    let c = codecs.trim().to_ascii_lowercase();
    if c.is_empty() {
        return 2;
    }
    if c.contains("mp4a") || c.contains("aac") {
        3
    } else if c.contains("opus") {
        0
    } else {
        1
    }
}

pub fn parse_dash_manifest(xml: &str) -> (Vec<DashTrack>, Vec<DashTrack>) {
    if xml.trim().is_empty() || !xml.to_ascii_lowercase().contains("<baseurl") {
        return (Vec::new(), Vec::new());
    }
    let mut videos = Vec::new();
    let mut audios = Vec::new();
    let sets = tag_blocks(xml, "AdaptationSet");
    let containers = if sets.is_empty() { vec![xml] } else { sets };

    for set in containers {
        let set_head = set.split('>').next().unwrap_or("");
        let set_mime = read_xml_attr(set_head, "mimeType")
            .or_else(|| read_xml_attr(set_head, "contentType"))
            .unwrap_or_default()
            .to_ascii_lowercase();

        for rep in tag_blocks(set, "Representation") {
            let rep_head = rep.split('>').next().unwrap_or("");
            let rep_mime = read_xml_attr(rep_head, "mimeType").unwrap_or_else(|| set_mime.clone());
            let codecs = read_xml_attr(rep_head, "codecs").unwrap_or_default();
            let width = read_xml_attr(rep_head, "width")
                .or_else(|| read_xml_attr(set_head, "width"))
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);
            let height = read_xml_attr(rep_head, "height")
                .or_else(|| read_xml_attr(set_head, "height"))
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);
            let bandwidth = read_xml_attr(rep_head, "bandwidth")
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);
            let Some(raw_url) = extract_tag_inner(rep, "BaseURL") else {
                continue;
            };
            let Some(url) = sanitize_media_url(&raw_url) else {
                continue;
            };
            let lower_codecs = codecs.to_ascii_lowercase();
            let is_audio = rep_mime.contains("audio")
                || lower_codecs.starts_with("mp4a")
                || lower_codecs.contains("opus")
                || (width == 0
                    && height == 0
                    && (set_mime.contains("audio")
                        || rep_head.to_ascii_lowercase().contains("audiosamplingrate")));
            let track = DashTrack {
                url,
                width,
                height,
                bandwidth,
                codecs,
                is_audio,
            };
            if is_audio {
                audios.push(track);
            } else {
                videos.push(track);
            }
        }
    }

    videos.sort_by(|a, b| {
        let area_a = (a.width as u64) * (a.height as u64);
        let area_b = (b.width as u64) * (b.height as u64);
        area_b
            .cmp(&area_a)
            .then_with(|| video_codec_rank(&b.codecs).cmp(&video_codec_rank(&a.codecs)))
            .then_with(|| b.bandwidth.cmp(&a.bandwidth))
    });
    audios.sort_by(|a, b| {
        audio_codec_rank(&b.codecs)
            .cmp(&audio_codec_rank(&a.codecs))
            .then_with(|| b.bandwidth.cmp(&a.bandwidth))
    });
    (videos, audios)
}

fn format_resolution_label(width: u32, height: u32, bitrate: u64, codecs: &str, suffix: &str) -> String {
    let codec_tag = {
        let c = codecs.trim().to_ascii_lowercase();
        if c.contains("avc") || c.contains("h264") {
            " H.264"
        } else if c.contains("hev") || c.contains("hvc") || c.contains("hevc") || c.contains("h265") || c.contains("bytevc1") {
            " HEVC"
        } else {
            ""
        }
    };
    let kbps = if bitrate >= 1000 {
        format!(" · {} kbps", bitrate / 1000)
    } else {
        String::new()
    };
    let short_side = width.min(height);
    if short_side > 0 {
        format!("{short_side}p MP4{codec_tag}{kbps}{suffix}")
    } else if width > 0 && height > 0 {
        format!("{width}x{height} MP4{codec_tag}{kbps}{suffix}")
    } else {
        format!("MP4{codec_tag}{kbps}{suffix}")
    }
}

fn push_unique_format(list: &mut Vec<MediaFormat>, candidate: MediaFormat) {
    if let Some(existing) = list
        .iter_mut()
        .find(|f| f.video_url == candidate.video_url && f.ext == candidate.ext)
    {
        for fb in candidate.fallback_urls {
            if !existing.fallback_urls.contains(&fb) && existing.video_url != fb {
                existing.fallback_urls.push(fb);
            }
        }
        return;
    }
    list.push(candidate);
}

fn url_list_from_addr(val: Option<&Value>) -> (Vec<String>, u32, u32) {
    let Some(v) = val else {
        return (Vec::new(), 0, 0);
    };
    if let Some(s) = v.as_str() {
        return (
            sanitize_media_url(s).into_iter().collect(),
            0,
            0,
        );
    }
    let width = v
        .get("Width")
        .or_else(|| v.get("width"))
        .and_then(|n| n.as_u64())
        .unwrap_or(0) as u32;
    let height = v
        .get("Height")
        .or_else(|| v.get("height"))
        .and_then(|n| n.as_u64())
        .unwrap_or(0) as u32;
    let mut urls = Vec::new();
    if let Some(arr) = v
        .get("UrlList")
        .or_else(|| v.get("url_list"))
        .or_else(|| v.get("urlList"))
        .and_then(|u| u.as_array())
    {
        for u in arr {
            if let Some(s) = u.as_str().and_then(sanitize_media_url) {
                if !urls.contains(&s) {
                    urls.push(s);
                }
            }
        }
    }
    (urls, width, height)
}

fn json_u64(v: &Value, keys: &[&str]) -> u64 {
    for k in keys {
        if let Some(val) = v.get(*k) {
            if let Some(n) = val.as_u64() {
                return n;
            }
            if let Some(s) = val.as_str() {
                if let Ok(n) = s.trim().parse::<u64>() {
                    return n;
                }
            }
        }
    }
    0
}

fn json_str<'a>(v: &'a Value, keys: &[&str]) -> Option<&'a str> {
    for k in keys {
        if let Some(s) = v.get(*k).and_then(|x| x.as_str()) {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }
    }
    None
}

fn normalize_tiktok_codec(raw: &str, is_bytevc1: bool) -> String {
    let lower = raw.trim().to_ascii_lowercase();
    if is_bytevc1 || lower.contains("h265") || lower.contains("hevc") || lower.contains("hvc1") || lower.contains("bytevc1") {
        "hvc1".to_string()
    } else if lower.contains("h264") || lower.contains("avc") {
        "avc1".to_string()
    } else {
        lower
    }
}

fn sort_and_annotate_video_formats(formats: &mut Vec<MediaFormat>, has_ffmpeg: bool) {
    formats.sort_by(|a, b| {
        if !has_ffmpeg {
            let prog_cmp = a.requires_mux.cmp(&b.requires_mux);
            if prog_cmp != std::cmp::Ordering::Equal {
                return prog_cmp;
            }
        }
        let area_a = (a.width as u64) * (a.height as u64);
        let area_b = (b.width as u64) * (b.height as u64);
        area_b
            .cmp(&area_a)
            .then_with(|| video_codec_rank(&b.codecs).cmp(&video_codec_rank(&a.codecs)))
            .then_with(|| b.bandwidth.cmp(&a.bandwidth))
    });
    if let Some(top) = formats.first_mut() {
        if !top.label.contains("Original") {
            top.label.push_str(" (Original)");
        }
    }
    if has_ffmpeg {
        if let Some(best_mp4) = formats.first().cloned() {
            push_unique_format(
                formats,
                MediaFormat {
                    id: "gif-export".to_string(),
                    label: "Animated GIF (.gif, 256-color Palette)".to_string(),
                    width: best_mp4.width,
                    height: best_mp4.height,
                    bandwidth: best_mp4.bandwidth,
                    codecs: "gif".to_string(),
                    ext: "gif".to_string(),
                    requires_mux: true,
                    video_url: best_mp4.video_url,
                    audio_url: None,
                    fallback_urls: best_mp4.fallback_urls,
                },
            );
        }
    }
}

fn extract_tiktok_video_item(node: &Value, has_ffmpeg: bool) -> Option<MediaItem> {
    let vid = node.get("video")?;
    let base_w = json_u64(vid, &["width", "Width"]) as u32;
    let base_h = json_u64(vid, &["height", "Height"]) as u32;
    let thumb = json_str(vid, &["originCover", "cover", "dynamicCover"])
        .and_then(sanitize_media_url)
        .or_else(|| {
            url_list_from_addr(vid.get("origin_cover").or_else(|| vid.get("cover")))
                .0
                .into_iter()
                .next()
        });

    let mut formats = Vec::new();
    let bitrate_arr = vid
        .get("bitrateInfo")
        .or_else(|| vid.get("bit_rate"))
        .and_then(|v| v.as_array());
    if let Some(arr) = bitrate_arr {
        for (idx, entry) in arr.iter().enumerate() {
            let (mut urls, w_addr, h_addr) =
                url_list_from_addr(entry.get("PlayAddr").or_else(|| entry.get("play_addr")));
            if urls.is_empty() {
                continue;
            }
            let primary = urls.remove(0);
            let w = if w_addr > 0 { w_addr } else { base_w };
            let h = if h_addr > 0 { h_addr } else { base_h };
            let bitrate = json_u64(entry, &["Bitrate", "bit_rate"]);
            let is_bytevc1 = json_u64(entry, &["is_bytevc1", "is_h265"]) == 1;
            let raw_codec = json_str(entry, &["CodecType", "codec_type"]).unwrap_or("");
            let codecs = normalize_tiktok_codec(raw_codec, is_bytevc1);
            push_unique_format(
                &mut formats,
                MediaFormat {
                    id: format!("tt-br-{idx}"),
                    label: format_resolution_label(w, h, bitrate, &codecs, ""),
                    width: w,
                    height: h,
                    bandwidth: if bitrate > 0 {
                        bitrate
                    } else {
                        (w as u64) * (h as u64)
                    },
                    codecs,
                    ext: "mp4".to_string(),
                    requires_mux: false,
                    video_url: primary,
                    audio_url: None,
                    fallback_urls: urls,
                },
            );
        }
    }

    for (tag, key) in [
        ("download", "downloadAddr"),
        ("download-snake", "download_addr"),
        ("play", "playAddr"),
        ("play-snake", "play_addr"),
    ] {
        let (mut urls, w_addr, h_addr) = url_list_from_addr(vid.get(key));
        if !urls.is_empty() {
            let primary = urls.remove(0);
            let w = if w_addr > 0 { w_addr } else { base_w };
            let h = if h_addr > 0 { h_addr } else { base_h };
            let bitrate = json_u64(vid, &["bitrate"]);
            push_unique_format(
                &mut formats,
                MediaFormat {
                    id: format!("tt-{tag}"),
                    label: format_resolution_label(w, h, bitrate, "", ""),
                    width: w,
                    height: h,
                    bandwidth: if bitrate > 0 {
                        bitrate
                    } else {
                        (w as u64) * (h as u64)
                    },
                    codecs: String::new(),
                    ext: "mp4".to_string(),
                    requires_mux: false,
                    video_url: primary,
                    audio_url: None,
                    fallback_urls: urls,
                },
            );
        }
    }

    if let Some(manifest) = json_str(vid, &["video_dash_manifest", "dashManifest"]) {
        let (dash_videos, dash_audios) = parse_dash_manifest(manifest);
        let best_audio = dash_audios.first().map(|a| a.url.clone());
        for (idx, track) in dash_videos.into_iter().enumerate() {
            let suffix = if best_audio.is_some() {
                if has_ffmpeg {
                    " (AAC Muxed)"
                } else {
                    " (DASH, needs FFmpeg for audio)"
                }
            } else {
                " (DASH)"
            };
            push_unique_format(
                &mut formats,
                MediaFormat {
                    id: format!("tt-dash-{idx}"),
                    label: format_resolution_label(
                        track.width,
                        track.height,
                        track.bandwidth,
                        &track.codecs,
                        suffix,
                    ),
                    width: track.width,
                    height: track.height,
                    bandwidth: track.bandwidth,
                    codecs: track.codecs,
                    ext: "mp4".to_string(),
                    requires_mux: best_audio.is_some(),
                    video_url: track.url,
                    audio_url: best_audio.clone(),
                    fallback_urls: Vec::new(),
                },
            );
        }
    }

    if formats.is_empty() {
        return None;
    }
    sort_and_annotate_video_formats(&mut formats, has_ffmpeg);
    Some(MediaItem {
        index: 0,
        kind: "video".to_string(),
        thumbnail_url: thumb,
        formats,
    })
}

fn infer_image_ext_from_url(url: &str) -> &'static str {
    let lower = url.to_ascii_lowercase();
    if lower.contains(".png") {
        "png"
    } else if lower.contains(".webp") {
        "webp"
    } else if lower.contains(".avif") {
        "avif"
    } else if lower.contains(".heic") {
        "heic"
    } else {
        "jpg"
    }
}

fn extract_tiktok_image_items(node: &Value) -> Vec<MediaItem> {
    let Some(images) = node
        .get("imagePost")
        .or_else(|| node.get("image_post_info"))
        .and_then(|ip| ip.get("images"))
        .and_then(|v| v.as_array())
    else {
        return Vec::new();
    };
    let mut items = Vec::new();
    for entry in images {
        let url_obj = entry
            .get("imageURL")
            .or_else(|| entry.get("display_image"))
            .or_else(|| entry.get("imageUrl"))
            .or_else(|| entry.get("thumbnail"));
        let (mut urls, w_addr, h_addr) = url_list_from_addr(url_obj);
        if urls.is_empty() {
            continue;
        }
        let primary = urls.remove(0);
        let w = json_u64(entry, &["imageWidth", "width"]) as u32;
        let h = json_u64(entry, &["imageHeight", "height"]) as u32;
        let width = if w > 0 { w } else { w_addr };
        let height = if h > 0 { h } else { h_addr };
        let ext = infer_image_ext_from_url(&primary);
        let label = if width > 0 && height > 0 {
            format!("{width}x{height} Original Image")
        } else {
            "Original Image".to_string()
        };
        let fmt = MediaFormat {
            id: format!("img-{}", items.len()),
            label,
            width,
            height,
            bandwidth: (width as u64) * (height as u64),
            codecs: String::new(),
            ext: ext.to_string(),
            requires_mux: false,
            video_url: primary.clone(),
            audio_url: None,
            fallback_urls: urls,
        };
        items.push(MediaItem {
            index: items.len(),
            kind: "image".to_string(),
            thumbnail_url: Some(primary),
            formats: vec![fmt],
        });
    }
    items
}

fn parse_ttnow_seed(
    root: &Value,
    video_hint: Option<&str>,
    has_ffmpeg: bool,
) -> Option<InspectedPost> {
    if !root
        .get("ttnowSeed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        return None;
    }
    let video_id = json_str(root, &["videoId", "id"])
        .map(str::to_string)
        .or_else(|| video_hint.map(str::to_string))
        .unwrap_or_default();
    let username = json_str(root, &["username", "uniqueId"])
        .unwrap_or("")
        .trim_start_matches('@')
        .to_string();
    let display_name = json_str(root, &["displayName", "nickname"])
        .unwrap_or("")
        .to_string();
    let caption = json_str(root, &["caption", "desc", "text"])
        .unwrap_or("")
        .to_string();
    let created_at = json_str(root, &["createdAt"]).unwrap_or("").to_string();
    let music_title = json_str(root, &["musicTitle"]).map(str::to_string);
    let music_author = json_str(root, &["musicAuthor"]).map(str::to_string);
    let music_url = json_str(root, &["musicUrl"]).and_then(sanitize_media_url);

    let mut items = Vec::new();
    if let Some(arr) = root.get("items").and_then(|v| v.as_array()) {
        for item in arr {
            let kind = json_str(item, &["kind"]).unwrap_or("image");
            let thumb = json_str(item, &["thumbnailUrl"]).and_then(sanitize_media_url);
            let mut urls: Vec<String> = item
                .get("urls")
                .and_then(|v| v.as_array())
                .map(|list| {
                    list.iter()
                        .filter_map(|u| u.as_str())
                        .filter_map(sanitize_media_url)
                        .collect()
                })
                .unwrap_or_default();
            if urls.is_empty() {
                if let Some(u) = json_str(item, &["url", "videoUrl"]).and_then(sanitize_media_url)
                {
                    urls.push(u);
                }
            }
            if urls.is_empty() {
                continue;
            }
            let width = json_u64(item, &["width"]) as u32;
            let height = json_u64(item, &["height"]) as u32;
            let mut formats = Vec::new();
            if kind == "video" {
                for (f_idx, u) in urls.into_iter().enumerate() {
                    push_unique_format(
                        &mut formats,
                        MediaFormat {
                            id: format!("seed-vid-{f_idx}"),
                            label: format_resolution_label(width, height, 0, "", ""),
                            width,
                            height,
                            bandwidth: (width as u64) * (height as u64),
                            codecs: String::new(),
                            ext: "mp4".to_string(),
                            requires_mux: false,
                            video_url: u,
                            audio_url: None,
                            fallback_urls: Vec::new(),
                        },
                    );
                }
                sort_and_annotate_video_formats(&mut formats, has_ffmpeg);
            } else {
                let primary = urls.remove(0);
                let ext = infer_image_ext_from_url(&primary);
                formats.push(MediaFormat {
                    id: format!("seed-img-{}", items.len()),
                    label: if width > 0 && height > 0 {
                        format!("{width}x{height} Original Image")
                    } else {
                        "Original Image".to_string()
                    },
                    width,
                    height,
                    bandwidth: (width as u64) * (height as u64),
                    codecs: String::new(),
                    ext: ext.to_string(),
                    requires_mux: false,
                    video_url: primary.clone(),
                    audio_url: None,
                    fallback_urls: urls,
                });
            }
            if !formats.is_empty() {
                let idx = items.len();
                items.push(MediaItem {
                    index: idx,
                    kind: if kind == "video" {
                        "video".to_string()
                    } else {
                        "image".to_string()
                    },
                    thumbnail_url: thumb.or_else(|| Some(formats[0].video_url.clone())),
                    formats,
                });
            }
        }
    }

    if items.is_empty() {
        return None;
    }
    let is_photo = items.iter().all(|i| i.kind == "image");
    let post_url = json_str(root, &["postUrl"])
        .filter(|s| is_safe_http_url(s) && is_tiktok_share_url(s))
        .map(str::to_string)
        .unwrap_or_else(|| canonical_post_url(&username, &video_id, is_photo));

    Some(InspectedPost {
        video_id,
        username,
        display_name,
        post_url,
        created_at,
        caption,
        music_title,
        music_author,
        music_url,
        ffmpeg_available: has_ffmpeg,
        items,
    })
}

fn node_is_tiktok_item(val: &Value) -> bool {
    let Some(obj) = val.as_object() else {
        return false;
    };
    let has_id = obj.get("id").and_then(|v| v.as_str()).is_some()
        || obj.get("aweme_id").and_then(|v| v.as_str()).is_some();
    if !has_id {
        return false;
    }
    obj.get("video").and_then(|v| v.as_object()).is_some()
        || obj.get("imagePost").and_then(|v| v.as_object()).is_some()
        || obj
            .get("image_post_info")
            .and_then(|v| v.as_object())
            .is_some()
}

fn build_post_from_node(
    node: &Value,
    video_hint: Option<&str>,
    username_hint: Option<&str>,
    has_ffmpeg: bool,
) -> Option<InspectedPost> {
    let image_items = extract_tiktok_image_items(node);
    let items = if !image_items.is_empty() {
        image_items
    } else {
        vec![extract_tiktok_video_item(node, has_ffmpeg)?]
    };

    let video_id = json_str(node, &["id", "aweme_id"])
        .map(str::to_string)
        .or_else(|| video_hint.map(str::to_string))
        .unwrap_or_default();

    let author_obj = node.get("author").filter(|v| v.is_object());
    let username = author_obj
        .and_then(|a| json_str(a, &["uniqueId", "unique_id", "shortId"]))
        .or_else(|| json_str(node, &["author", "authorName"]))
        .or(username_hint)
        .unwrap_or("")
        .trim_start_matches('@')
        .to_string();
    let display_name = author_obj
        .and_then(|a| json_str(a, &["nickname", "name"]))
        .or_else(|| json_str(node, &["nickname"]))
        .unwrap_or("")
        .to_string();

    let caption = json_str(node, &["desc", "caption", "title"])
        .unwrap_or("")
        .to_string();

    let create_epoch = json_u64(node, &["createTime", "create_time"]);
    let created_at = if create_epoch > 0 {
        format_unix_iso_utc(create_epoch)
    } else {
        String::new()
    };

    let music_obj = node.get("music").filter(|v| v.is_object());
    let music_title = music_obj
        .and_then(|m| json_str(m, &["title"]))
        .map(str::to_string);
    let music_author = music_obj
        .and_then(|m| json_str(m, &["authorName", "author"]))
        .map(str::to_string);
    let music_url = music_obj
        .and_then(|m| {
            json_str(m, &["playUrl"])
                .and_then(sanitize_media_url)
                .or_else(|| {
                    url_list_from_addr(m.get("play_url"))
                        .0
                        .into_iter()
                        .next()
                })
        });

    let is_photo = items.iter().all(|i| i.kind == "image");
    let post_url = canonical_post_url(&username, &video_id, is_photo);

    Some(InspectedPost {
        video_id,
        username,
        display_name,
        post_url,
        created_at,
        caption,
        music_title,
        music_author,
        music_url,
        ffmpeg_available: has_ffmpeg,
        items,
    })
}

fn scan_value_for_post(
    root: &Value,
    video_hint: Option<&str>,
    username_hint: Option<&str>,
    has_ffmpeg: bool,
) -> Option<InspectedPost> {
    if let Some(seed) = parse_ttnow_seed(root, video_hint, has_ffmpeg) {
        return Some(seed);
    }
    let mut stack = vec![(root, 0usize)];
    let mut visited = 0usize;
    let mut fallback: Option<InspectedPost> = None;

    while let Some((val, depth)) = stack.pop() {
        if visited >= 24_000 || depth > 28 {
            continue;
        }
        visited += 1;
        if node_is_tiktok_item(val) {
            if let Some(post) = build_post_from_node(val, video_hint, username_hint, has_ffmpeg) {
                if video_hint
                    .map(|hint| !hint.is_empty() && post.video_id == hint)
                    .unwrap_or(false)
                {
                    return Some(post);
                }
                if fallback.is_none() {
                    fallback = Some(post);
                }
            }
        }
        match val {
            Value::Array(arr) => {
                for item in arr.iter().rev() {
                    if item.is_object() || item.is_array() {
                        stack.push((item, depth + 1));
                    }
                }
            }
            Value::Object(map) => {
                for child in map.values() {
                    if child.is_object() || child.is_array() {
                        stack.push((child, depth + 1));
                    }
                }
            }
            _ => {}
        }
    }
    fallback
}

pub fn extract_post_from_json_text_with_user(
    raw: &str,
    video_hint: Option<&str>,
    username_hint: Option<&str>,
    has_ffmpeg: bool,
) -> Option<InspectedPost> {
    let cleaned = raw.trim();
    if cleaned.is_empty() || cleaned.len() > 12 * 1024 * 1024 {
        return None;
    }
    if let Ok(parsed) = serde_json::from_str::<Value>(cleaned) {
        if let Some(post) = scan_value_for_post(&parsed, video_hint, username_hint, has_ffmpeg) {
            return Some(post);
        }
    }
    None
}

#[allow(dead_code)]
pub fn extract_post_from_json_text(
    raw: &str,
    video_hint: Option<&str>,
    has_ffmpeg: bool,
) -> Option<InspectedPost> {
    extract_post_from_json_text_with_user(raw, video_hint, None, has_ffmpeg)
}

fn find_meta_content(html: &str, attr_name: &str, attr_value: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let target = attr_value.to_ascii_lowercase();
    let mut pos = 0;
    while let Some(idx) = lower[pos..].find("<meta") {
        let abs = pos + idx;
        let Some(gt_rel) = lower[abs..].find('>') else {
            break;
        };
        let tag = &html[abs..abs + gt_rel + 1];
        pos = abs + gt_rel + 1;
        let matches = read_xml_attr(tag, attr_name)
            .map(|v| v.eq_ignore_ascii_case(&target))
            .unwrap_or(false);
        if matches {
            if let Some(content) = read_xml_attr(tag, "content") {
                return Some(unescape_url(&content));
            }
        }
    }
    None
}

fn find_script_inner_by_id<'a>(html: &'a str, id_name: &str) -> Option<&'a str> {
    let lower = html.to_ascii_lowercase();
    let needle = format!("id=\"{}\"", id_name.to_ascii_lowercase());
    let needle_sq = format!("id='{}'", id_name.to_ascii_lowercase());
    let idx = lower.find(&needle).or_else(|| lower.find(&needle_sq))?;
    let gt = lower[idx..].find('>')? + idx + 1;
    let end = lower[gt..].find("</script>")? + gt;
    Some(html[gt..end].trim())
}

pub fn extract_post_from_html_with_user(
    html: &str,
    video_hint: Option<&str>,
    username_hint: Option<&str>,
    has_ffmpeg: bool,
) -> Option<InspectedPost> {
    for script_id in ["__UNIVERSAL_DATA_FOR_REHYDRATION__", "SIGI_STATE"] {
        if let Some(inner) = find_script_inner_by_id(html, script_id) {
            if let Some(post) = extract_post_from_json_text_with_user(
                inner,
                video_hint,
                username_hint,
                has_ffmpeg,
            ) {
                return Some(post);
            }
        }
    }

    for block in tag_blocks(html, "script") {
        let Some(body_start) = block.find('>') else {
            continue;
        };
        let inner = block[body_start + 1..]
            .rsplit_once("</")
            .map(|(head, _)| head)
            .unwrap_or("")
            .trim();
        if inner.contains("itemStruct")
            || inner.contains("ItemModule")
            || inner.contains("playAddr")
            || inner.contains("bitrateInfo")
            || inner.contains("imagePost")
            || inner.contains("aweme_detail")
        {
            if let Some(post) = extract_post_from_json_text_with_user(
                inner,
                video_hint,
                username_hint,
                has_ffmpeg,
            ) {
                return Some(post);
            }
        }
    }

    let video_url = find_meta_content(html, "property", "og:video:secure_url")
        .or_else(|| find_meta_content(html, "property", "og:video:url"))
        .or_else(|| find_meta_content(html, "property", "og:video"))
        .and_then(|u| sanitize_media_url(&u));
    let image_url = find_meta_content(html, "property", "og:image")
        .or_else(|| find_meta_content(html, "name", "twitter:image"))
        .and_then(|u| sanitize_media_url(&u));
    let caption = find_meta_content(html, "property", "og:description")
        .or_else(|| find_meta_content(html, "name", "description"))
        .unwrap_or_default()
        .trim()
        .to_string();

    let mut items = Vec::new();
    if let Some(url) = video_url {
        items.push(MediaItem {
            index: 0,
            kind: "video".to_string(),
            thumbnail_url: image_url,
            formats: vec![MediaFormat {
                id: "og-video".to_string(),
                label: "MP4 Video".to_string(),
                width: 0,
                height: 0,
                bandwidth: 0,
                codecs: String::new(),
                ext: "mp4".to_string(),
                requires_mux: false,
                video_url: url,
                audio_url: None,
                fallback_urls: Vec::new(),
            }],
        });
    } else if let Some(url) = image_url {
        let ext = infer_image_ext_from_url(&url);
        items.push(MediaItem {
            index: 0,
            kind: "image".to_string(),
            thumbnail_url: Some(url.clone()),
            formats: vec![MediaFormat {
                id: "og-image".to_string(),
                label: "Original Image".to_string(),
                width: 0,
                height: 0,
                bandwidth: 0,
                codecs: String::new(),
                ext: ext.to_string(),
                requires_mux: false,
                video_url: url,
                audio_url: None,
                fallback_urls: Vec::new(),
            }],
        });
    }

    if items.is_empty() {
        return None;
    }

    let video_id = video_hint.unwrap_or("").to_string();
    let username = username_hint
        .unwrap_or("")
        .trim_start_matches('@')
        .to_string();
    let is_photo = items.iter().all(|i| i.kind == "image");
    let post_url = canonical_post_url(&username, &video_id, is_photo);

    Some(InspectedPost {
        video_id,
        username,
        display_name: String::new(),
        post_url,
        created_at: String::new(),
        caption,
        music_title: None,
        music_author: None,
        music_url: None,
        ffmpeg_available: has_ffmpeg,
        items,
    })
}

#[allow(dead_code)]
pub fn extract_post_from_html(
    html: &str,
    video_hint: Option<&str>,
    has_ffmpeg: bool,
) -> Option<InspectedPost> {
    extract_post_from_html_with_user(html, video_hint, None, has_ffmpeg)
}

pub fn extract_oembed_metadata(raw: &str, video_hint: Option<&str>) -> Option<InspectedPost> {
    let val: Value = serde_json::from_str(raw.trim()).ok()?;
    let caption = json_str(&val, &["title"]).unwrap_or("").to_string();
    let display_name = json_str(&val, &["author_name"]).unwrap_or("").to_string();
    let username = json_str(&val, &["author_unique_id"])
        .or_else(|| {
            json_str(&val, &["author_url"])
                .and_then(|u| u.rsplit('/').find(|s| !s.is_empty()))
                .map(|s| s.trim_start_matches('@'))
        })
        .unwrap_or("")
        .trim_start_matches('@')
        .to_string();
    let video_id = json_str(&val, &["embed_product_id"])
        .map(str::to_string)
        .or_else(|| video_hint.map(str::to_string))
        .unwrap_or_default();
    if username.is_empty() && caption.is_empty() && display_name.is_empty() {
        return None;
    }
    let post_url = canonical_post_url(&username, &video_id, false);
    Some(InspectedPost {
        video_id,
        username,
        display_name,
        post_url,
        created_at: String::new(),
        caption,
        music_title: None,
        music_author: None,
        music_url: None,
        ffmpeg_available: find_ffmpeg_binary().is_some(),
        items: Vec::new(),
    })
}

fn post_richness_score(post: &InspectedPost) -> (usize, usize, u64, usize) {
    let item_count = post.items.len();
    let fmt_count: usize = post.items.iter().map(|i| i.formats.len()).sum();
    let max_area: u64 = post
        .items
        .iter()
        .flat_map(|i| i.formats.iter())
        .map(|f| (f.width as u64) * (f.height as u64))
        .max()
        .unwrap_or(0);
    let meta_score = usize::from(!post.username.trim().is_empty())
        + usize::from(!post.display_name.trim().is_empty())
        + usize::from(!post.caption.trim().is_empty())
        + usize::from(!post.created_at.trim().is_empty())
        + usize::from(post.music_url.is_some());
    (item_count, fmt_count, max_area, meta_score)
}

/// Merge two inspected post representations, keeping the richest media items and filling any empty metadata.
pub fn merge_inspected_post(primary: InspectedPost, secondary: InspectedPost) -> InspectedPost {
    let (mut rich, other) = if post_richness_score(&secondary) > post_richness_score(&primary) {
        (secondary, primary)
    } else {
        (primary, secondary)
    };
    if rich.video_id.is_empty() {
        rich.video_id = other.video_id;
    }
    if rich.username.is_empty() {
        rich.username = other.username;
    }
    if rich.display_name.is_empty() {
        rich.display_name = other.display_name;
    }
    if rich.created_at.is_empty() {
        rich.created_at = other.created_at;
    }
    if rich.caption.is_empty() {
        rich.caption = other.caption;
    }
    if rich.music_title.is_none() {
        rich.music_title = other.music_title;
    }
    if rich.music_author.is_none() {
        rich.music_author = other.music_author;
    }
    if rich.music_url.is_none() {
        rich.music_url = other.music_url;
    }
    if rich.post_url.is_empty() || rich.post_url == "https://www.tiktok.com/" {
        if !other.post_url.is_empty() {
            rich.post_url = other.post_url;
        } else if !rich.video_id.is_empty() {
            let is_photo = !rich.items.is_empty() && rich.items.iter().all(|i| i.kind == "image");
            rich.post_url = canonical_post_url(&rich.username, &rich.video_id, is_photo);
        }
    }
    rich.ffmpeg_available = rich.ffmpeg_available || other.ffmpeg_available;
    rich
}

/// Choose the richer post between HTTP inspection and a page-side JS seed, merging metadata when IDs match.
pub fn pick_richer_inspected_post(
    http_post: InspectedPost,
    seed_post: InspectedPost,
) -> InspectedPost {
    let same_id = http_post.video_id.is_empty()
        || seed_post.video_id.is_empty()
        || http_post.video_id == seed_post.video_id;
    let seed_has_formats = seed_post.items.iter().any(|item| !item.formats.is_empty());
    if !same_id || !seed_has_formats {
        return http_post;
    }
    merge_inspected_post(http_post, seed_post)
}

pub fn is_complete_inspected_post(
    post: &InspectedPost,
    requested_video_id: Option<&str>,
) -> bool {
    if post.username.trim().is_empty() || post.video_id.trim().is_empty() || post.items.is_empty() {
        return false;
    }
    if let Some(vid) = requested_video_id.map(str::trim).filter(|s| !s.is_empty()) {
        if post.video_id != vid {
            return false;
        }
    }
    post.items.iter().all(|item| {
        !item.formats.is_empty()
            && item
                .formats
                .iter()
                .all(|fmt| !fmt.video_url.trim().is_empty())
    })
}

fn format_unix_iso_utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let hour = rem / 3_600;
    let minute = (rem % 3_600) / 60;
    let second = rem % 60;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    format!("{year:04}-{m:02}-{d:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn format_unix_yyyymmdd_utc(secs: u64) -> String {
    let iso = format_unix_iso_utc(secs);
    iso.chars().take(10).filter(|c| c.is_ascii_digit()).collect()
}

fn find_ffmpeg_in_paths_and_candidates(
    path_var: Option<&OsStr>,
    candidates: &[PathBuf],
) -> Option<PathBuf> {
    let names = if cfg!(windows) {
        &["ffmpeg.exe", "ffmpeg"][..]
    } else {
        &["ffmpeg"][..]
    };
    if let Some(paths) = path_var {
        for dir in env::split_paths(paths) {
            for name in names {
                let candidate = dir.join(name);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    candidates.iter().find(|c| c.is_file()).cloned()
}

pub fn find_ffmpeg_binary() -> Option<PathBuf> {
    #[cfg(windows)]
    let extra = {
        let mut list = vec![
            PathBuf::from(r"C:\ProgramData\chocolatey\bin\ffmpeg.exe"),
            PathBuf::from(r"C:\ProgramData\HP\LCDDisplayHelper\bin\ffmpeg.exe"),
        ];
        if let Ok(local_app) = env::var("LOCALAPPDATA") {
            list.push(PathBuf::from(local_app).join(r"Microsoft\WinGet\Links\ffmpeg.exe"));
        }
        if let Ok(profile) = env::var("USERPROFILE") {
            list.push(PathBuf::from(profile).join(r"scoop\shims\ffmpeg.exe"));
        }
        list
    };
    #[cfg(not(windows))]
    let extra = vec![
        PathBuf::from("/opt/homebrew/bin/ffmpeg"),
        PathBuf::from("/usr/local/bin/ffmpeg"),
        PathBuf::from("/opt/local/bin/ffmpeg"),
        PathBuf::from("/usr/bin/ffmpeg"),
    ];
    find_ffmpeg_in_paths_and_candidates(env::var_os("PATH").as_deref(), &extra)
}

pub fn ffmpeg_available() -> bool {
    find_ffmpeg_binary().is_some()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mp4AudioState {
    Aac,
    NonAac,
    Missing,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Mp4Layout {
    pub is_mp4: bool,
    pub has_ftyp: bool,
    pub has_moov: bool,
    pub has_mdat: bool,
    pub is_fragmented: bool,
    pub moov_before_mdat: bool,
    pub is_faststart: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaStreamsInfo {
    pub container: String,
    pub video_codec: String,
    pub pix_fmt: String,
    pub audio_codec: String,
    pub audio_state: Mp4AudioState,
    pub layout: Mp4Layout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormatKind {
    Jpg,
    Png,
    Gif,
    Webp,
    Avif,
    Heic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageFormat {
    pub kind: ImageFormatKind,
    pub ext: &'static str,
    pub is_lossless_or_alpha: bool,
}

/// Detect HTML error/challenge pages or JSON error payloads returned instead of binary media.
pub fn looks_like_html_error(bytes: &[u8]) -> bool {
    let trimmed = bytes
        .iter()
        .copied()
        .skip_while(|b| b.is_ascii_whitespace() || matches!(b, 0xEF | 0xBB | 0xBF))
        .take(64)
        .collect::<Vec<u8>>();
    if trimmed.is_empty() {
        return false;
    }
    let lower = String::from_utf8_lossy(&trimmed).to_ascii_lowercase();
    lower.starts_with("<!doctype")
        || lower.starts_with("<html")
        || lower.starts_with("<head")
        || lower.starts_with("<title")
        || lower.starts_with("<script")
        || lower.starts_with("<?xml")
}

pub fn is_valid_mp4_header(bytes: &[u8]) -> bool {
    bytes.len() >= 8
        && matches!(
            &bytes[4..8],
            b"ftyp" | b"styp" | b"wide" | b"free" | b"moov"
        )
}

pub fn is_valid_webm_header(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && bytes[0..4] == [0x1a, 0x45, 0xdf, 0xa3]
}

pub fn is_valid_download_video_header(bytes: &[u8], allow_webm: bool) -> bool {
    !looks_like_html_error(bytes)
        && (is_valid_mp4_header(bytes) || (allow_webm && is_valid_webm_header(bytes)))
}

pub fn detect_image_format(bytes: &[u8]) -> Option<ImageFormat> {
    if bytes.len() < 6 || looks_like_html_error(bytes) {
        return None;
    }
    if bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
        return Some(ImageFormat {
            kind: ImageFormatKind::Jpg,
            ext: "jpg",
            is_lossless_or_alpha: false,
        });
    }
    if bytes.len() >= 8 && &bytes[0..8] == b"\x89PNG\r\n\x1a\n" {
        return Some(ImageFormat {
            kind: ImageFormatKind::Png,
            ext: "png",
            is_lossless_or_alpha: true,
        });
    }
    if &bytes[0..6] == b"GIF87a" || &bytes[0..6] == b"GIF89a" {
        return Some(ImageFormat {
            kind: ImageFormatKind::Gif,
            ext: "gif",
            is_lossless_or_alpha: true,
        });
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        let chunk = bytes.get(12..16).unwrap_or(b"");
        let is_lossless_or_alpha =
            chunk == b"VP8L" || (chunk == b"VP8X" && bytes.len() >= 21 && (bytes[20] & 0x10) != 0);
        return Some(ImageFormat {
            kind: ImageFormatKind::Webp,
            ext: "webp",
            is_lossless_or_alpha,
        });
    }
    if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" {
        const IMAGE_BRANDS: &[&str] = &[
            "avif", "avis", "heic", "heix", "hevc", "hevx", "mif1", "msf1",
        ];
        let mut brands = vec![String::from_utf8_lossy(&bytes[8..12]).to_ascii_lowercase()];
        let limit = bytes.len().min(64);
        let mut i = 16;
        while i + 4 <= limit {
            brands.push(String::from_utf8_lossy(&bytes[i..i + 4]).to_ascii_lowercase());
            i += 4;
        }
        if !brands.iter().any(|b| IMAGE_BRANDS.contains(&b.as_str())) {
            return None;
        }
        let is_avif = brands.iter().any(|b| b == "avif" || b == "avis");
        return Some(if is_avif {
            ImageFormat {
                kind: ImageFormatKind::Avif,
                ext: "avif",
                is_lossless_or_alpha: false,
            }
        } else {
            ImageFormat {
                kind: ImageFormatKind::Heic,
                ext: "heic",
                is_lossless_or_alpha: false,
            }
        });
    }
    None
}

pub fn is_valid_image_header(bytes: &[u8]) -> bool {
    detect_image_format(bytes).is_some()
}

#[allow(dead_code)]
pub fn sniff_image_extension(bytes: &[u8]) -> &'static str {
    detect_image_format(bytes).map(|f| f.ext).unwrap_or("jpg")
}

pub fn replace_path_extension(path: &Path, new_ext: &str) -> PathBuf {
    path.with_extension(new_ext.trim_start_matches('.'))
}

fn read_file_head(path: &Path, max_bytes: usize) -> Result<Vec<u8>, String> {
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut buf = vec![0_u8; max_bytes];
    let n = file.read(&mut buf).map_err(|e| e.to_string())?;
    buf.truncate(n);
    Ok(buf)
}

fn split_top_level_comma(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (idx, ch) in text.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' if depth > 0 => depth -= 1,
            ',' if depth == 0 => {
                parts.push(text[start..idx].trim());
                start = idx + 1;
            }
            _ => {}
        }
    }
    parts.push(text[start..].trim());
    parts
}

pub fn inspect_mp4_layout(path: &Path) -> Mp4Layout {
    let mut result = Mp4Layout::default();
    let Ok(mut file) = fs::File::open(path) else {
        return result;
    };
    let Ok(meta) = file.metadata() else {
        return result;
    };
    let file_size = meta.len();
    let mut pos: u64 = 0;
    let mut first_box: Option<[u8; 4]> = None;
    let mut moov_pos: Option<u64> = None;
    let mut mdat_pos: Option<u64> = None;
    let mut header = [0_u8; 16];

    for _ in 0..4096 {
        if pos + 8 > file_size {
            break;
        }
        if file.seek(SeekFrom::Start(pos)).is_err() {
            break;
        }
        let Ok(n) = file.read(&mut header[..8]) else {
            break;
        };
        if n < 8 {
            break;
        }
        let size32 = u32::from_be_bytes([header[0], header[1], header[2], header[3]]);
        let box_type: [u8; 4] = [header[4], header[5], header[6], header[7]];
        if !box_type.iter().all(|&b| (0x20..=0x7e).contains(&b)) {
            break;
        }
        let box_size: u64 = if size32 == 1 {
            if pos + 16 > file_size {
                break;
            }
            if file.read_exact(&mut header[8..16]).is_err() {
                break;
            }
            let big = u64::from_be_bytes([
                header[8], header[9], header[10], header[11], header[12], header[13], header[14],
                header[15],
            ]);
            if big < 16 || pos.saturating_add(big) > file_size {
                break;
            }
            big
        } else if size32 == 0 {
            file_size - pos
        } else {
            u64::from(size32)
        };

        if box_size < 8 || pos.saturating_add(box_size) > file_size {
            break;
        }
        if first_box.is_none() {
            first_box = Some(box_type);
        }
        if &box_type == b"ftyp" {
            result.has_ftyp = true;
        } else if &box_type == b"moov" && moov_pos.is_none() {
            result.has_moov = true;
            moov_pos = Some(pos);
        } else if &box_type == b"mdat" && mdat_pos.is_none() {
            result.has_mdat = true;
            mdat_pos = Some(pos);
        } else if &box_type == b"moof" {
            result.is_fragmented = true;
        }
        pos += box_size;
    }

    if first_box.as_ref() == Some(b"styp") {
        result.is_fragmented = true;
    }
    result.is_mp4 = result.has_ftyp
        || matches!(
            first_box.as_ref(),
            Some(b"styp" | b"moov" | b"wide" | b"free")
        );
    result.moov_before_mdat = match (moov_pos, mdat_pos) {
        (Some(moov), Some(mdat)) => moov < mdat,
        (Some(_), None) => true,
        _ => false,
    };
    result.is_faststart =
        result.has_ftyp && result.has_moov && !result.is_fragmented && result.moov_before_mdat;
    result
}

pub fn probe_media_streams(ffmpeg: &Path, path: &Path) -> MediaStreamsInfo {
    let mut layout = inspect_mp4_layout(path);
    let mut info = MediaStreamsInfo {
        container: String::new(),
        video_codec: String::new(),
        pix_fmt: String::new(),
        audio_codec: String::new(),
        audio_state: Mp4AudioState::Missing,
        layout: layout.clone(),
    };
    let mut cmd = Command::new(ffmpeg);
    apply_no_window(&mut cmd);
    let Ok(out) = cmd.arg("-i").arg(path).output() else {
        return info;
    };
    let stderr = String::from_utf8_lossy(&out.stderr);
    for line in stderr.lines() {
        let trimmed = line.trim_start();
        if info.container.is_empty() {
            if let Some(after) = trimmed.strip_prefix("Input #0,") {
                if let Some(from_idx) = after.find(" from ") {
                    info.container = after[..from_idx].trim().to_ascii_lowercase();
                }
            }
        }
        if info.video_codec.is_empty() {
            if let Some(v_pos) = line.find("Video:") {
                let parts = split_top_level_comma(line[v_pos + 6..].trim());
                if let Some(first) = parts.first() {
                    let codec: String = first
                        .to_ascii_lowercase()
                        .chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '+' | '-'))
                        .collect();
                    if !codec.is_empty() {
                        info.video_codec = codec;
                    }
                }
                if let Some(second) = parts.get(1) {
                    let pix: String = second
                        .to_ascii_lowercase()
                        .chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                        .collect();
                    if !pix.is_empty() {
                        info.pix_fmt = pix;
                    }
                }
            }
        }
        if info.audio_codec.is_empty() {
            if let Some(a_pos) = line.find("Audio:") {
                let tail = line[a_pos + 6..].trim().to_ascii_lowercase();
                let codec: String = tail
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '+' | '-'))
                    .collect();
                if !codec.is_empty() {
                    info.audio_state = if codec == "aac" {
                        Mp4AudioState::Aac
                    } else {
                        Mp4AudioState::NonAac
                    };
                    info.audio_codec = codec;
                }
            }
        }
    }
    if !layout.is_mp4 && (info.container.contains("mp4") || info.container.contains("mov")) {
        layout.is_mp4 = true;
    }
    info.layout = layout;
    info
}

fn run_ffmpeg_to_atomic_output<I, S>(
    ffmpeg: &Path,
    args: I,
    out_path: &Path,
    tag: &str,
    ext: &str,
    label: &str,
) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let temp_out = unique_temp_media_path_near(out_path, tag, ext);
    let mut cmd = Command::new(ffmpeg);
    apply_no_window(&mut cmd);
    let output = cmd
        .args(args)
        .arg(&temp_out)
        .output()
        .map_err(|e| format!("Failed to start ffmpeg: {e}"))?;
    if !output.status.success() {
        let _ = fs::remove_file(&temp_out);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let tail: String = stderr.lines().rev().take(4).collect::<Vec<_>>().join(" | ");
        return Err(format!(
            "ffmpeg {label} exited with status {}: {tail}",
            output.status
        ));
    }
    let meta = fs::metadata(&temp_out).map_err(|e| {
        let _ = fs::remove_file(&temp_out);
        e.to_string()
    })?;
    if meta.len() == 0 {
        let _ = fs::remove_file(&temp_out);
        return Err(format!("ffmpeg {label} produced an empty output file."));
    }
    replace_file_atomically(&temp_out, out_path)?;
    Ok(())
}

fn paths_refer_to_same_file(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    if !b.exists() {
        return false;
    }
    matches!(
        (fs::canonicalize(a), fs::canonicalize(b)),
        (Ok(ca), Ok(cb)) if ca == cb
    )
}

pub fn mux_video_audio_ffmpeg_with_streams(
    ffmpeg: &Path,
    video_path: &Path,
    audio_path: &Path,
    out_path: &Path,
    preprobed_video: Option<&MediaStreamsInfo>,
) -> Result<(), String> {
    let owned_v;
    let v_streams = match preprobed_video {
        Some(s) => s,
        None => {
            owned_v = probe_media_streams(ffmpeg, video_path);
            &owned_v
        }
    };
    let a_streams = probe_media_streams(ffmpeg, audio_path);
    let video_ok = v_streams.video_codec == "h264" && v_streams.pix_fmt == "yuv420p";
    let audio_ok = a_streams.audio_state == Mp4AudioState::Aac;

    let mut args: Vec<std::ffi::OsString> = vec![
        "-y".into(),
        "-fflags".into(),
        "+genpts".into(),
        "-i".into(),
        video_path.as_os_str().to_os_string(),
        "-i".into(),
        audio_path.as_os_str().to_os_string(),
        "-map".into(),
        "0:v:0".into(),
        "-map".into(),
        "1:a:0".into(),
    ];
    if video_ok {
        args.extend(["-c:v".into(), "copy".into()]);
    } else {
        args.extend([
            "-c:v".into(),
            "libx264".into(),
            "-crf".into(),
            "16".into(),
            "-preset".into(),
            "medium".into(),
            "-pix_fmt".into(),
            "yuv420p".into(),
        ]);
    }
    if audio_ok {
        args.extend(["-c:a".into(), "copy".into()]);
    } else {
        args.extend([
            "-c:a".into(),
            "aac".into(),
            "-b:a".into(),
            "192k".into(),
            "-ac".into(),
            "2".into(),
            "-ar".into(),
            "44100".into(),
        ]);
    }
    args.extend(["-shortest".into(), "-movflags".into(), "+faststart".into()]);
    run_ffmpeg_to_atomic_output(ffmpeg, args, out_path, "mux", "mp4", "muxing")
}

pub fn mux_video_audio_ffmpeg(
    ffmpeg: &Path,
    video_path: &Path,
    audio_path: &Path,
    out_path: &Path,
) -> Result<(), String> {
    mux_video_audio_ffmpeg_with_streams(ffmpeg, video_path, audio_path, out_path, None)
}

pub fn normalize_video_mp4_with_streams(
    ffmpeg: &Path,
    input_path: &Path,
    fallback_audio_path: Option<&Path>,
    out_path: &Path,
    preprobed: Option<&MediaStreamsInfo>,
) -> Result<Option<String>, String> {
    let owned;
    let streams = match preprobed {
        Some(s) => s,
        None => {
            owned = probe_media_streams(ffmpeg, input_path);
            &owned
        }
    };
    if streams.video_codec.is_empty() {
        return Err("The downloaded video contained no decodable video stream.".to_string());
    }
    if streams.audio_state == Mp4AudioState::Missing {
        if let Some(aud) = fallback_audio_path {
            mux_video_audio_ffmpeg_with_streams(ffmpeg, input_path, aud, out_path, Some(streams))?;
            return Ok(None);
        }
    }

    let video_ok = streams.video_codec == "h264" && streams.pix_fmt == "yuv420p";
    let has_audio = streams.audio_state != Mp4AudioState::Missing;
    let audio_ok = streams.audio_state == Mp4AudioState::Aac;
    let container_ok =
        streams.layout.is_mp4 && !streams.layout.is_fragmented && streams.layout.is_faststart;
    let warning = if has_audio {
        None
    } else {
        Some("This TikTok video stream had no audio track.".to_string())
    };

    if video_ok && (audio_ok || !has_audio) && container_ok {
        if !paths_refer_to_same_file(input_path, out_path) {
            let temp_copy = unique_temp_media_path_near(out_path, "vcopy", "mp4");
            if let Err(err) = fs::copy(input_path, &temp_copy) {
                let _ = fs::remove_file(&temp_copy);
                return Err(err.to_string());
            }
            if let Err(err) = replace_file_atomically(&temp_copy, out_path) {
                let _ = fs::remove_file(&temp_copy);
                return Err(err);
            }
        }
        return Ok(warning);
    }

    let mut args: Vec<std::ffi::OsString> = vec![
        "-y".into(),
        "-fflags".into(),
        "+genpts".into(),
        "-i".into(),
        input_path.as_os_str().to_os_string(),
        "-map".into(),
        "0:v:0".into(),
    ];
    if has_audio {
        args.extend(["-map".into(), "0:a:0".into()]);
    }
    if video_ok {
        args.extend(["-c:v".into(), "copy".into()]);
    } else {
        args.extend([
            "-c:v".into(),
            "libx264".into(),
            "-crf".into(),
            "16".into(),
            "-preset".into(),
            "medium".into(),
            "-pix_fmt".into(),
            "yuv420p".into(),
        ]);
    }
    if has_audio {
        if audio_ok {
            args.extend(["-c:a".into(), "copy".into()]);
        } else {
            args.extend([
                "-c:a".into(),
                "aac".into(),
                "-b:a".into(),
                "192k".into(),
                "-ac".into(),
                "2".into(),
                "-ar".into(),
                "44100".into(),
            ]);
        }
    }
    args.extend(["-movflags".into(), "+faststart".into()]);
    run_ffmpeg_to_atomic_output(ffmpeg, args, out_path, "vnorm", "mp4", "normalization")?;
    Ok(warning)
}

pub fn normalize_video_mp4(
    ffmpeg: &Path,
    input_path: &Path,
    fallback_audio_path: Option<&Path>,
    out_path: &Path,
) -> Result<Option<String>, String> {
    normalize_video_mp4_with_streams(ffmpeg, input_path, fallback_audio_path, out_path, None)
}

/// Extract or encode an audio-only `.m4a` (AAC stereo 192 kbps) track from any video or audio file.
pub fn extract_audio_aac_m4a_ffmpeg_with_streams(
    ffmpeg: &Path,
    input_path: &Path,
    out_path: &Path,
    preprobed: Option<&MediaStreamsInfo>,
) -> Result<(), String> {
    let owned;
    let streams = match preprobed {
        Some(s) => s,
        None => {
            owned = probe_media_streams(ffmpeg, input_path);
            &owned
        }
    };
    let mut args: Vec<std::ffi::OsString> = vec![
        "-y".into(),
        "-fflags".into(),
        "+genpts".into(),
        "-i".into(),
        input_path.as_os_str().to_os_string(),
        "-vn".into(),
        "-sn".into(),
        "-dn".into(),
        "-map".into(),
        "0:a:0".into(),
    ];
    if streams.audio_state == Mp4AudioState::Aac && streams.layout.is_mp4 {
        args.extend(["-c:a".into(), "copy".into()]);
    } else {
        args.extend([
            "-c:a".into(),
            "aac".into(),
            "-b:a".into(),
            "192k".into(),
            "-ac".into(),
            "2".into(),
            "-ar".into(),
            "44100".into(),
        ]);
    }
    args.extend(["-movflags".into(), "+faststart".into()]);
    run_ffmpeg_to_atomic_output(ffmpeg, args, out_path, "m4a", "m4a", "audio extraction")
}

pub fn extract_audio_aac_m4a_ffmpeg(
    ffmpeg: &Path,
    input_path: &Path,
    out_path: &Path,
) -> Result<(), String> {
    extract_audio_aac_m4a_ffmpeg_with_streams(ffmpeg, input_path, out_path, None)
}

/// Export a video file as an infinite-loop animated GIF using a per-frame 256-color palette.
pub fn convert_mp4_to_gif_ffmpeg(
    ffmpeg: &Path,
    input_mp4: &Path,
    out_gif: &Path,
) -> Result<(), String> {
    let args: Vec<std::ffi::OsString> = vec![
        "-y".into(),
        "-i".into(),
        input_mp4.as_os_str().to_os_string(),
        "-vf".into(),
        "split[s0][s1];[s0]palettegen=max_colors=256:stats_mode=diff[p];[s1][p]paletteuse=dither=floyd_steinberg".into(),
        "-loop".into(),
        "0".into(),
    ];
    run_ffmpeg_to_atomic_output(ffmpeg, args, out_gif, "gif", "gif", "GIF conversion")
}

#[allow(dead_code)]
pub fn convert_image_ffmpeg(
    ffmpeg: &Path,
    input_path: &Path,
    out_path: &Path,
    target_ext: &str,
) -> Result<(), String> {
    let mut args: Vec<std::ffi::OsString> = vec![
        "-y".into(),
        "-i".into(),
        input_path.as_os_str().to_os_string(),
        "-frames:v".into(),
        "1".into(),
    ];
    if target_ext.eq_ignore_ascii_case("jpg") || target_ext.eq_ignore_ascii_case("jpeg") {
        args.extend(["-q:v".into(), "1".into(), "-qmin".into(), "1".into()]);
    } else if target_ext.eq_ignore_ascii_case("png") {
        args.extend(["-c:v".into(), "png".into(), "-compression_level".into(), "6".into()]);
    }
    let ext = target_ext.trim_start_matches('.');
    run_ffmpeg_to_atomic_output(ffmpeg, args, out_path, "imgconv", ext, "image conversion")
}

pub fn download_hls_to_mp4_ffmpeg(
    ffmpeg: &Path,
    hls_url: &str,
    out_mp4: &Path,
) -> Result<(), String> {
    if !is_safe_http_url(hls_url) || !is_tiktok_media_url(hls_url) {
        return Err("Invalid TikTok HLS stream URL.".to_string());
    }
    let args: Vec<std::ffi::OsString> = vec![
        "-y".into(),
        "-user_agent".into(),
        DEFAULT_UA.into(),
        "-referer".into(),
        "https://www.tiktok.com/".into(),
        "-i".into(),
        hls_url.into(),
        "-c".into(),
        "copy".into(),
        "-bsf:a".into(),
        "aac_adtstoasc".into(),
        "-movflags".into(),
        "+faststart".into(),
    ];
    run_ffmpeg_to_atomic_output(ffmpeg, args, out_mp4, "hls", "mp4", "HLS download")
}

fn is_alpha_or_lossless_pix_fmt(pix_fmt: &str) -> bool {
    let lower = pix_fmt.to_ascii_lowercase();
    [
        "rgba", "bgra", "argb", "abgr", "yuva", "gbrap", "pal8", "ya8", "ya16",
    ]
    .iter()
    .any(|pat| lower.contains(pat))
}

/// Sniff downloaded image bytes and pass through JPG/PNG/GIF verbatim, converting WebP/AVIF/HEIC only when FFmpeg is present.
pub fn normalize_image_file(
    ffmpeg: Option<&Path>,
    input_path: &Path,
    target_path: &Path,
) -> Result<PathBuf, String> {
    let head = read_file_head(input_path, 64)?;
    let detected = detect_image_format(&head).ok_or_else(|| {
        "TikTok returned a non-image response (expired link or HTML error page).".to_string()
    })?;

    let passthrough = matches!(
        detected.kind,
        ImageFormatKind::Jpg | ImageFormatKind::Png | ImageFormatKind::Gif
    ) || ffmpeg.is_none();

    if passthrough {
        let final_path = replace_path_extension(target_path, detected.ext);
        if !paths_refer_to_same_file(input_path, &final_path) {
            let temp_copy = unique_temp_media_path_near(&final_path, "imgcopy", detected.ext);
            if let Err(err) = fs::copy(input_path, &temp_copy) {
                let _ = fs::remove_file(&temp_copy);
                return Err(err.to_string());
            }
            if let Err(err) = replace_file_atomically(&temp_copy, &final_path) {
                let _ = fs::remove_file(&temp_copy);
                return Err(err);
            }
        }
        return Ok(final_path);
    }

    let ff = ffmpeg.expect("checked above");
    let use_png = detected.is_lossless_or_alpha || {
        let streams = probe_media_streams(ff, input_path);
        is_alpha_or_lossless_pix_fmt(&streams.pix_fmt)
    };
    let out_ext = if use_png { "png" } else { "jpg" };
    let final_path = replace_path_extension(target_path, out_ext);
    let mut args: Vec<std::ffi::OsString> = vec![
        "-y".into(),
        "-i".into(),
        input_path.as_os_str().to_os_string(),
        "-frames:v".into(),
        "1".into(),
    ];
    if use_png {
        args.extend(["-c:v".into(), "png".into(), "-compression_level".into(), "6".into()]);
    } else {
        args.extend([
            "-c:v".into(),
            "mjpeg".into(),
            "-q:v".into(),
            "1".into(),
            "-qmin".into(),
            "1".into(),
        ]);
    }
    run_ffmpeg_to_atomic_output(
        ff,
        args,
        &final_path,
        "imgconv",
        out_ext,
        "image conversion",
    )?;
    Ok(final_path)
}

pub fn sanitize_path_segment(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches(|c| c == '.' || c == '_');
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." {
        String::new()
    } else {
        let capped: String = trimmed.chars().take(80).collect();
        if is_windows_reserved_stem(&capped) {
            format!("_{capped}")
        } else {
            capped
        }
    }
}

pub fn build_source_filename_stem(username: &str, created_at: &str, video_id: &str) -> String {
    let cleaned_user: String = username
        .trim()
        .trim_start_matches('@')
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let user_part = cleaned_user.trim_matches(|c| c == '.' || c == '_');
    let user = if user_part.is_empty() {
        "tiktok"
    } else {
        user_part
    };
    let mut user_capped: String = user.chars().take(36).collect();
    if is_windows_reserved_stem(&user_capped) {
        user_capped = format!("_{user_capped}");
    }

    let date_digits: String = created_at
        .chars()
        .take(10)
        .filter(|c| c.is_ascii_digit())
        .collect();
    let date_part = if date_digits.len() == 8 {
        date_digits
    } else {
        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        format_unix_yyyymmdd_utc(now_secs)
    };

    let id_part: String = video_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
        .take(24)
        .collect();
    if id_part.is_empty() {
        let seq = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
        format!("{user_capped}-{date_part}-{seq}")
    } else {
        format!("{user_capped}-{date_part}-{id_part}")
    }
}

pub fn item_slide_stem(base: &str, item_index: usize, item_count: usize) -> String {
    if item_count >= 10 {
        format!("{base}-slide-{:02}", item_index + 1)
    } else if item_count > 1 {
        format!("{base}-slide-{}", item_index + 1)
    } else {
        base.to_string()
    }
}

pub fn format_caption_document(post: &InspectedPost) -> String {
    let mut lines = Vec::new();
    if !post.username.trim().is_empty() {
        if post.display_name.trim().is_empty() {
            lines.push(format!("Author: @{}", post.username.trim()));
        } else {
            lines.push(format!(
                "Author: {} (@{})",
                post.display_name.trim(),
                post.username.trim()
            ));
        }
    }
    if !post.post_url.trim().is_empty() {
        lines.push(format!("URL: {}", post.post_url.trim()));
    }
    if !post.created_at.trim().is_empty() {
        lines.push(format!("Date: {}", post.created_at.trim()));
    }
    if let Some(title) = post.music_title.as_deref().filter(|s| !s.trim().is_empty()) {
        let music_line = match post.music_author.as_deref().filter(|s| !s.trim().is_empty()) {
            Some(author) => format!("Sound: {} — {}", title.trim(), author.trim()),
            None => format!("Sound: {}", title.trim()),
        };
        lines.push(music_line);
    }
    if !lines.is_empty() {
        lines.push(String::new());
    }
    let body = post.caption.trim();
    lines.push(if body.is_empty() {
        "(No caption text available for this post)".to_string()
    } else {
        body.to_string()
    });
    lines.join("\n")
}

pub fn build_metadata_json(post: &InspectedPost) -> Result<String, String> {
    let payload = serde_json::json!({
        "videoId": post.video_id,
        "username": post.username,
        "displayName": post.display_name,
        "postUrl": post.post_url,
        "createdAt": post.created_at,
        "caption": post.caption,
        "musicTitle": post.music_title,
        "musicAuthor": post.music_author,
        "mediaCount": post.items.len(),
    });
    serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())
}

pub fn cookie_header_from_pairs(pairs: &[(String, String)]) -> Option<String> {
    let mut seen = HashSet::new();
    let mut cookie_parts = Vec::new();
    for (k, v) in pairs {
        let name: String = k
            .chars()
            .filter(|c| !matches!(c, '\r' | '\n' | '\0'))
            .collect();
        let val: String = v
            .chars()
            .filter(|c| !matches!(c, '\r' | '\n' | '\0'))
            .collect();
        if name.is_empty() || val.is_empty() || !seen.insert(name.clone()) {
            continue;
        }
        cookie_parts.push(format!("{name}={val}"));
    }
    (!cookie_parts.is_empty()).then(|| cookie_parts.join("; "))
}

#[allow(dead_code)]
pub fn cookie_header_and_csrf_from_pairs(
    pairs: &[(String, String)],
) -> (Option<String>, Option<String>) {
    (cookie_header_from_pairs(pairs), None)
}

pub fn tiktok_auth_cookies(app: &AppHandle, main_window_label: &str) -> Option<String> {
    let mut pairs = Vec::new();
    if let Some(win) = app.get_webview_window(main_window_label) {
        if let Ok(tt_url) = "https://www.tiktok.com/".parse() {
            if let Ok(cookies) = win.cookies_for_url(tt_url) {
                for cookie in cookies {
                    pairs.push((cookie.name().to_string(), cookie.value().to_string()));
                }
            }
        }
    }
    cookie_header_from_pairs(&pairs)
}

fn escape_curl_config_header(header_name: &str, value: &str) -> Option<String> {
    let clean: String = value
        .chars()
        .filter(|&c| c != '\r' && c != '\n' && c != '\0')
        .collect();
    let trimmed = clean.trim();
    if trimmed.is_empty() {
        return None;
    }
    let escaped = trimmed.replace('\\', "\\\\").replace('"', "\\\"");
    Some(format!("header = \"{header_name}: {escaped}\"\n"))
}

pub fn build_curl_stdin_headers(cookie_header: Option<&str>) -> Vec<u8> {
    let mut buf = String::new();
    if let Some(cookie) = cookie_header {
        if let Some(line) = escape_curl_config_header("Cookie", cookie) {
            buf.push_str(&line);
        }
    }
    buf.into_bytes()
}

pub fn run_curl_with_auth_stdin(
    cmd: &mut Command,
    url: &str,
    cookie_header: Option<&str>,
    _csrf_token: Option<&str>,
) -> std::io::Result<Output> {
    let send_auth = is_tiktok_app_host(url);
    let config_bytes = if send_auth {
        build_curl_stdin_headers(cookie_header)
    } else {
        Vec::new()
    };
    if !config_bytes.is_empty() {
        cmd.args(["--config", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = cmd.spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(&config_bytes);
        }
        return child.wait_with_output();
    }
    cmd.output()
}

pub fn curl_download_file(
    url: &str,
    referer: &str,
    ua: &str,
    cookie_header: Option<&str>,
    target_path: &Path,
) -> Result<u64, String> {
    if !is_safe_http_url(url) {
        return Err("TikTok did not provide a valid HTTP(S) media URL.".to_string());
    }
    let curl = system_curl_bin();
    let mut cmd = Command::new(&curl);
    apply_no_window(&mut cmd);
    cmd.args([
        "--fail",
        "--location",
        "--proto",
        "=http,https",
        "--proto-redir",
        "=http,https",
        "--max-redirs",
        "5",
        "--silent",
        "--show-error",
        "--retry",
        "2",
        "--connect-timeout",
        "20",
        "--max-time",
        "300",
        "--user-agent",
    ])
    .arg(ua)
    .arg("--referer")
    .arg(referer)
    .args(["--header", "Accept: */*", "--output"])
    .arg(target_path)
    .arg(url);
    let output = run_curl_with_auth_stdin(&mut cmd, url, cookie_header, None)
        .map_err(|error| format!("Unable to start {curl}: {error}"))?;
    if !output.status.success() {
        let _ = fs::remove_file(target_path);
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "{curl} exited with status {}: {}",
            output.status,
            stderr.trim()
        ));
    }
    let metadata = fs::metadata(target_path).map_err(|error| error.to_string())?;
    if metadata.len() == 0 {
        let _ = fs::remove_file(target_path);
        return Err("The downloaded media file was empty.".to_string());
    }
    let head = read_file_head(target_path, 64).unwrap_or_default();
    if looks_like_html_error(&head) {
        let _ = fs::remove_file(target_path);
        return Err("TikTok returned an HTML error page instead of media.".to_string());
    }
    Ok(metadata.len())
}

pub fn run_curl_text(
    url: &str,
    cookie_header: Option<&str>,
    referer: Option<&str>,
) -> Result<String, String> {
    if !is_safe_http_url(url) {
        return Err("Invalid TikTok URL.".to_string());
    }
    let curl = system_curl_bin();
    let mut cmd = Command::new(&curl);
    apply_no_window(&mut cmd);
    cmd.args([
        "--fail",
        "--location",
        "--proto",
        "=http,https",
        "--proto-redir",
        "=http,https",
        "--max-redirs",
        "5",
        "--silent",
        "--show-error",
        "--compressed",
        "--connect-timeout",
        "10",
        "--max-time",
        "15",
        "--user-agent",
        DEFAULT_UA,
        "--referer",
        referer.unwrap_or("https://www.tiktok.com/"),
        "--header",
        "Accept-Language: en-US,en;q=0.9",
        url,
    ]);
    let output = run_curl_with_auth_stdin(&mut cmd, url, cookie_header, None)
        .map_err(|e| format!("Failed to invoke {curl}: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("HTTP error ({url}): {}", stderr.trim()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub fn resolve_shortlink_url(url: &str, cookie_header: Option<&str>) -> Option<String> {
    if !is_safe_http_url(url) || !is_tiktok_share_url(url) {
        return None;
    }
    let curl = system_curl_bin();
    let mut cmd = Command::new(&curl);
    apply_no_window(&mut cmd);
    cmd.args([
        "--location",
        "--proto",
        "=http,https",
        "--proto-redir",
        "=http,https",
        "--max-redirs",
        "6",
        "--silent",
        "--show-error",
        "--connect-timeout",
        "8",
        "--max-time",
        "12",
        "--user-agent",
        DEFAULT_UA,
        "--output",
        if cfg!(windows) { "NUL" } else { "/dev/null" },
        "--write-out",
        "%{url_effective}",
        url,
    ]);
    let output = run_curl_with_auth_stdin(&mut cmd, url, cookie_header, None).ok()?;
    if !output.status.success() {
        return None;
    }
    let resolved = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (is_safe_http_url(&resolved) && is_tiktok_share_url(&resolved)).then_some(resolved)
}

fn percent_encode_component(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for b in input.as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(*b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

/// Inspect a TikTok post/photo/shortlink over HTTP (shortlink resolve -> HTML rehydration/SIGI/og -> embed -> oEmbed enrichment).
pub fn inspect_via_http_cascade(
    url_or_id: &str,
    cookie_header: Option<&str>,
    _csrf_token: Option<&str>,
) -> Result<InspectedPost, String> {
    let trimmed = url_or_id.trim();
    if trimmed.is_empty() {
        return Err("Enter a valid TikTok video or photo URL.".to_string());
    }

    let effective_url = if is_tiktok_shortlink(trimmed) {
        resolve_shortlink_url(trimmed, cookie_header).unwrap_or_else(|| trimmed.to_string())
    } else {
        trimmed.to_string()
    };

    let video_id = video_id_from_url(&effective_url).ok_or_else(|| {
        "Enter a valid TikTok post URL (tiktok.com/@user/video/... or /photo/...) or video ID."
            .to_string()
    })?;
    let user_hint = username_from_url(&effective_url);
    let has_ffmpeg = find_ffmpeg_binary().is_some();

    let page_url = if is_safe_http_url(&effective_url) && is_tiktok_app_host(&effective_url) {
        effective_url.clone()
    } else {
        canonical_post_url(user_hint.as_deref().unwrap_or(""), &video_id, false)
    };

    let mut best: Option<InspectedPost> = None;

    if let Ok(html) = run_curl_text(&page_url, cookie_header, Some("https://www.tiktok.com/")) {
        if let Some(post) = extract_post_from_html_with_user(
            &html,
            Some(&video_id),
            user_hint.as_deref(),
            has_ffmpeg,
        ) {
            best = Some(post);
        }
    }

    if best.as_ref().is_none_or(|p| p.items.is_empty()) {
        let embed_url = format!("https://www.tiktok.com/embed/v2/{video_id}");
        if let Ok(html) = run_curl_text(&embed_url, cookie_header, Some("https://www.tiktok.com/")) {
            if let Some(post) = extract_post_from_html_with_user(
                &html,
                Some(&video_id),
                user_hint.as_deref(),
                has_ffmpeg,
            ) {
                best = Some(match best {
                    Some(prev) => merge_inspected_post(prev, post),
                    None => post,
                });
            }
        }
    }

    if let Some(mut post) = best {
        if post.username.is_empty() || post.caption.is_empty() || post.display_name.is_empty() {
            let oembed_target = if is_safe_http_url(&post.post_url) {
                &post.post_url
            } else {
                &page_url
            };
            let oembed_url = format!(
                "https://www.tiktok.com/oembed?url={}",
                percent_encode_component(oembed_target)
            );
            if let Ok(raw) = run_curl_text(&oembed_url, None, Some("https://www.tiktok.com/")) {
                if let Some(meta_post) = extract_oembed_metadata(&raw, Some(&video_id)) {
                    post = merge_inspected_post(post, meta_post);
                }
            }
        }
        if !post.items.is_empty() {
            return Ok(post);
        }
    }

    Err("TikTok did not return playable media from HTTP endpoints.".to_string())
}

async fn probe_post_via_tiktok_window(
    app: &AppHandle,
    probe_state: &DownloaderProbeState,
    main_window_label: &str,
    clean_input: &str,
    video_id: Option<&str>,
    username_hint: Option<&str>,
    has_ffmpeg: bool,
) -> Option<InspectedPost> {
    let tt_win = app.get_webview_window(main_window_label)?;
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let seq = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let request_id = format!("probe-{nanos}-{seq}");
    let (tx, rx) = oneshot::channel();
    probe_state.register_probe(request_id.clone(), tx);
    let _guard = ProbeCancelGuard::new(probe_state, request_id.clone());

    let url_json = serde_json::to_string(clean_input).ok()?;
    let req_json = serde_json::to_string(&request_id).ok()?;
    let script = format!(
        "if (window.__tiktoknowTools && window.__tiktoknowTools.probePostForDownloader) window.__tiktoknowTools.probePostForDownloader({req_json}, {url_json});"
    );
    if tt_win.eval(&script).is_err() {
        return None;
    }
    let waited = tokio::time::timeout(std::time::Duration::from_secs(15), rx).await;
    if let Ok(Ok(raw_payload)) = waited {
        return extract_post_from_json_text_with_user(
            &raw_payload,
            video_id,
            username_hint,
            has_ffmpeg,
        )
        .or_else(|| {
            extract_post_from_html_with_user(&raw_payload, video_id, username_hint, has_ffmpeg)
        });
    }
    None
}

/// Inspect a TikTok post using page seed, Rust HTTP cascade, and live WebView probe fallback.
pub async fn inspect_tiktok_post_http(
    app: &AppHandle,
    probe_state: &DownloaderProbeState,
    main_window_label: &str,
    url: String,
    seed_payload_json: Option<String>,
) -> Result<InspectedPost, String> {
    let clean_input = url.trim().to_string();
    let video_id = video_id_from_url(&clean_input);
    let user_hint = username_from_url(&clean_input);
    let has_ffmpeg = ffmpeg_available();

    let seed_post = seed_payload_json
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .and_then(|seed| {
            extract_post_from_json_text_with_user(
                seed,
                video_id.as_deref(),
                user_hint.as_deref(),
                has_ffmpeg,
            )
        });

    if let Some(ref post) = seed_post {
        if is_complete_inspected_post(post, video_id.as_deref()) {
            return Ok(post.clone());
        }
    }

    let cookie_header = tiktok_auth_cookies(app, main_window_label);
    let input_for_http = clean_input.clone();
    let mut http_handle = tokio::task::spawn_blocking(move || {
        inspect_via_http_cascade(&input_for_http, cookie_header.as_deref(), None)
    });
    let probe_fut = probe_post_via_tiktok_window(
        app,
        probe_state,
        main_window_label,
        &clean_input,
        video_id.as_deref(),
        user_hint.as_deref(),
        has_ffmpeg,
    );
    tokio::pin!(probe_fut);

    tokio::select! {
        probe_res = &mut probe_fut => {
            if let Some(probed) = probe_res {
                if is_complete_inspected_post(&probed, video_id.as_deref()) {
                    return Ok(match seed_post {
                        Some(seed) => pick_richer_inspected_post(probed, seed),
                        None => probed,
                    });
                }
                let http_res = http_handle.await.map_err(|e| e.to_string())?;
                if let Ok(post) = http_res {
                    let best = match seed_post {
                        Some(seed) => pick_richer_inspected_post(post, seed),
                        None => post,
                    };
                    return Ok(pick_richer_inspected_post(best, probed));
                }
                return Ok(match seed_post {
                    Some(seed) => pick_richer_inspected_post(probed, seed),
                    None => probed,
                });
            }
            let http_res = http_handle.await.map_err(|e| e.to_string())?;
            if let Ok(post) = http_res {
                return Ok(match seed_post {
                    Some(seed) => pick_richer_inspected_post(post, seed),
                    None => post,
                });
            }
            if let Some(post) = seed_post {
                return Ok(post);
            }
            http_res
        }
        http_joined = &mut http_handle => {
            let http_res = http_joined.map_err(|e| e.to_string())?;
            if let Ok(post) = http_res {
                let mut best = match seed_post {
                    Some(seed) => pick_richer_inspected_post(post, seed),
                    None => post,
                };
                if best.username.is_empty() && best.items.len() <= 1 {
                    if let Some(probed) = probe_fut.await {
                        best = pick_richer_inspected_post(best, probed);
                    }
                }
                return Ok(best);
            }
            if let Some(post) = seed_post {
                return Ok(post);
            }
            if let Some(post) = probe_fut.await {
                return Ok(post);
            }
            http_res
        }
    }
}

pub fn unique_temp_media_path(tag: &str, ext: &str) -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let seq = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
    env::temp_dir().join(format!("ttnow-{tag}-{ts}-{seq}.{ext}"))
}

pub fn unique_temp_media_path_near(target: &Path, tag: &str, ext: &str) -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let seq = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(env::temp_dir);
    dir.join(format!(".ttnow-{tag}-{ts}-{seq}.{ext}"))
}

pub fn replace_file_atomically(temp_path: &Path, target_path: &Path) -> Result<(), String> {
    if fs::rename(temp_path, target_path).is_ok() {
        return Ok(());
    }
    let backup = unique_temp_media_path_near(target_path, "bak", "tmp");
    let had_backup = target_path.exists() && fs::rename(target_path, &backup).is_ok();
    let res = match fs::rename(temp_path, target_path) {
        Ok(()) => Ok(()),
        Err(rename_err) => match fs::copy(temp_path, target_path) {
            Ok(_) => Ok(()),
            Err(copy_err) => Err(format!(
                "Failed to replace destination file ({rename_err}; {copy_err})"
            )),
        },
    };
    let _ = fs::remove_file(temp_path);
    if res.is_ok() {
        if had_backup {
            let _ = fs::remove_file(&backup);
        }
    } else if had_backup {
        let _ = fs::rename(&backup, target_path);
    }
    res
}

fn media_extension(media_type: &str) -> &'static str {
    if media_type.eq_ignore_ascii_case("video") {
        "mp4"
    } else if media_type.eq_ignore_ascii_case("gif") {
        "gif"
    } else if media_type.eq_ignore_ascii_case("audio")
        || media_type.eq_ignore_ascii_case("audio-m4a")
        || media_type.eq_ignore_ascii_case("m4a")
    {
        "m4a"
    } else if media_type.eq_ignore_ascii_case("mp3") || media_type.eq_ignore_ascii_case("audio-mp3")
    {
        "mp3"
    } else if media_type.eq_ignore_ascii_case("text")
        || media_type.eq_ignore_ascii_case("caption")
        || media_type.eq_ignore_ascii_case("txt")
    {
        "txt"
    } else if media_type.eq_ignore_ascii_case("json") {
        "json"
    } else {
        "jpg"
    }
}

pub fn prepare_download_folder(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .download_dir()
        .map_err(|e| e.to_string())?
        .join("TikTok-Now");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

pub fn open_downloads_folder(app: &AppHandle) -> Result<(), String> {
    let dir = prepare_download_folder(app)?;
    open::that(&dir).map_err(|e| e.to_string())
}

fn target_path_in_downloads(
    app: &AppHandle,
    ext: &str,
    subfolder: Option<&str>,
    file_stem: Option<&str>,
    allow_overwrite: bool,
) -> Result<PathBuf, String> {
    let mut dir = prepare_download_folder(app)?;
    if let Some(sub) = subfolder
        .map(sanitize_path_segment)
        .filter(|s| !s.is_empty())
    {
        dir = dir.join(sub);
    }
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stem = file_stem
        .map(sanitize_path_segment)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            format!("TikTok-Now-{ts}")
        });
    let clean_ext = ext.trim_start_matches('.');
    let mut candidate = dir.join(format!("{stem}.{clean_ext}"));
    if !allow_overwrite && candidate.exists() {
        let mut n = 2u32;
        while candidate.exists() && n < 1000 {
            candidate = dir.join(format!("{stem}-{n}.{clean_ext}"));
            n += 1;
        }
    }
    Ok(candidate)
}

fn validate_or_remove_video_input(path: &Path, allow_webm: bool) -> Result<(), String> {
    let head = read_file_head(path, 64).inspect_err(|_| {
        let _ = fs::remove_file(path);
    })?;
    if !is_valid_download_video_header(&head, allow_webm) {
        let _ = fs::remove_file(path);
        return Err(
            "TikTok returned an invalid video stream (media fragment or HTML error page)."
                .to_string(),
        );
    }
    Ok(())
}

fn validate_or_remove_mp4(path: &Path) -> Result<(), String> {
    validate_or_remove_video_input(path, false)
}

fn validate_or_remove_image(path: &Path) -> Result<(), String> {
    let head = read_file_head(path, 64).inspect_err(|_| {
        let _ = fs::remove_file(path);
    })?;
    if !is_valid_image_header(&head) {
        let _ = fs::remove_file(path);
        return Err(
            "TikTok returned a non-image response (expired link or HTML error page).".to_string(),
        );
    }
    Ok(())
}

fn collect_item_fallback_candidates(item: &MediaItem, primary_url: &str) -> Vec<(String, Option<String>)> {
    let mut list = Vec::new();
    for fmt in &item.formats {
        if fmt.ext == "gif" {
            continue;
        }
        if fmt.video_url != primary_url {
            list.push((fmt.video_url.clone(), fmt.audio_url.clone()));
        }
        for fb in &fmt.fallback_urls {
            if fb != primary_url && !list.iter().any(|(u, _)| u == fb) {
                list.push((fb.clone(), fmt.audio_url.clone()));
            }
        }
    }
    list
}

fn select_fallback_audio(
    formats: &[MediaFormat],
    chosen_audio: Option<&str>,
    music_url: Option<&str>,
) -> Option<String> {
    formats
        .iter()
        .filter_map(|f| f.audio_url.as_deref())
        .chain(music_url)
        .map(str::trim)
        .find(|fb| !fb.is_empty() && chosen_audio != Some(*fb))
        .map(str::to_string)
}

#[allow(clippy::too_many_arguments)]
fn fetch_media_to_path(
    url: &str,
    media_type: &str,
    target_ext: &str,
    audio_url: Option<&str>,
    fallback_audio_url: Option<&str>,
    referer: &str,
    ua: &str,
    cookie_header: Option<&str>,
    target_path: &Path,
) -> Result<(PathBuf, Option<String>), String> {
    let clean_audio = audio_url.map(str::trim).filter(|s| is_safe_http_url(s));
    let clean_fallback = fallback_audio_url
        .map(str::trim)
        .filter(|s| is_safe_http_url(s));

    if media_type.eq_ignore_ascii_case("gif") || target_ext.eq_ignore_ascii_case("gif") {
        let ffmpeg = find_ffmpeg_binary()
            .ok_or_else(|| "FFmpeg is required to export Animated GIF (.gif) files.".to_string())?;
        let temp_v = unique_temp_media_path("gif-src", "bin");
        if url.contains(".m3u8") {
            download_hls_to_mp4_ffmpeg(&ffmpeg, url, &temp_v)?;
        } else if let Err(err) = curl_download_file(url, referer, ua, cookie_header, &temp_v) {
            let _ = fs::remove_file(&temp_v);
            return Err(err);
        }
        validate_or_remove_video_input(&temp_v, true)?;
        let res = convert_mp4_to_gif_ffmpeg(&ffmpeg, &temp_v, target_path);
        let _ = fs::remove_file(&temp_v);
        res?;
        return Ok((target_path.to_path_buf(), None));
    }

    if media_type.eq_ignore_ascii_case("video") {
        let ffmpeg = find_ffmpeg_binary();
        if url.contains(".m3u8") {
            let ff = ffmpeg
                .ok_or_else(|| "FFmpeg is required to download HLS video streams.".to_string())?;
            download_hls_to_mp4_ffmpeg(&ff, url, target_path)?;
            validate_or_remove_mp4(target_path)?;
            return Ok((target_path.to_path_buf(), None));
        }
        if let Some(a_url) = clean_audio {
            if let Some(ref ff) = ffmpeg {
                let temp_v = unique_temp_media_path("video", "bin");
                let temp_a = unique_temp_media_path("audio", "bin");
                curl_download_file(url, referer, ua, cookie_header, &temp_v)?;
                validate_or_remove_video_input(&temp_v, true)?;
                let audio_ok = curl_download_file(a_url, referer, ua, cookie_header, &temp_a);
                let mux_ok = if audio_ok.is_ok() {
                    mux_video_audio_ffmpeg(ff, &temp_v, &temp_a, target_path)
                } else {
                    Err("Audio stream download failed".to_string())
                };
                let _ = fs::remove_file(&temp_a);
                if mux_ok.is_ok() {
                    let _ = fs::remove_file(&temp_v);
                    validate_or_remove_mp4(target_path)?;
                    return Ok((target_path.to_path_buf(), None));
                }
                let mut temp_fb: Option<PathBuf> = None;
                if let Some(fb_url) = clean_fallback.filter(|fb| *fb != a_url) {
                    let candidate = unique_temp_media_path("fallback-audio", "bin");
                    if curl_download_file(fb_url, referer, ua, cookie_header, &candidate).is_ok() {
                        temp_fb = Some(candidate);
                    } else {
                        let _ = fs::remove_file(&candidate);
                    }
                }
                let norm_res = normalize_video_mp4(ff, &temp_v, temp_fb.as_deref(), target_path);
                let _ = fs::remove_file(&temp_v);
                if let Some(ref p) = temp_fb {
                    let _ = fs::remove_file(p);
                }
                let warn = norm_res?;
                validate_or_remove_mp4(target_path)?;
                return Ok((
                    target_path.to_path_buf(),
                    warn.map(|_| {
                        "Saved .mp4 video stream only; DASH audio muxing failed.".to_string()
                    }),
                ));
            }
            curl_download_file(url, referer, ua, cookie_header, target_path)?;
            validate_or_remove_mp4(target_path)?;
            return Ok((
                target_path.to_path_buf(),
                Some(
                    "Saved .mp4 video stream only; install ffmpeg to combine DASH audio."
                        .to_string(),
                ),
            ));
        }

        if let Some(ref ff) = ffmpeg {
            let temp_v = unique_temp_media_path("video", "bin");
            if let Err(err) = curl_download_file(url, referer, ua, cookie_header, &temp_v) {
                let _ = fs::remove_file(&temp_v);
                return Err(err);
            }
            validate_or_remove_video_input(&temp_v, true)?;
            let mut temp_fb: Option<PathBuf> = None;
            let preprobed = if let Some(fb_url) = clean_fallback {
                let streams = probe_media_streams(ff, &temp_v);
                if streams.audio_state == Mp4AudioState::Missing {
                    let candidate = unique_temp_media_path("fallback-audio", "bin");
                    if curl_download_file(fb_url, referer, ua, cookie_header, &candidate).is_ok() {
                        temp_fb = Some(candidate);
                    } else {
                        let _ = fs::remove_file(&candidate);
                    }
                }
                Some(streams)
            } else {
                None
            };
            let ensure_res = normalize_video_mp4_with_streams(
                ff,
                &temp_v,
                temp_fb.as_deref(),
                target_path,
                preprobed.as_ref(),
            );
            let _ = fs::remove_file(&temp_v);
            if let Some(ref p) = temp_fb {
                let _ = fs::remove_file(p);
            }
            let warn = ensure_res?;
            validate_or_remove_mp4(target_path)?;
            return Ok((target_path.to_path_buf(), warn));
        }

        curl_download_file(url, referer, ua, cookie_header, target_path)?;
        validate_or_remove_mp4(target_path)?;
        return Ok((target_path.to_path_buf(), None));
    }

    if media_type.eq_ignore_ascii_case("image") {
        let temp_img = unique_temp_media_path_near(target_path, "imgdl", "tmp");
        if let Err(err) = curl_download_file(url, referer, ua, cookie_header, &temp_img) {
            let _ = fs::remove_file(&temp_img);
            return Err(err);
        }
        validate_or_remove_image(&temp_img)?;
        let res = normalize_image_file(find_ffmpeg_binary().as_deref(), &temp_img, target_path);
        let _ = fs::remove_file(&temp_img);
        let final_path = res?;
        return Ok((final_path, None));
    }

    curl_download_file(url, referer, ua, cookie_header, target_path)?;
    Ok((target_path.to_path_buf(), None))
}

#[allow(clippy::too_many_arguments)]
fn fetch_media_with_fallbacks(
    primary_url: &str,
    fallback_candidates: &[(String, Option<String>)],
    media_type: &str,
    target_ext: &str,
    audio_url: Option<&str>,
    fallback_audio_url: Option<&str>,
    referer: &str,
    ua: &str,
    cookie_header: Option<&str>,
    target_path: &Path,
) -> Result<(PathBuf, Option<String>), String> {
    let primary = fetch_media_to_path(
        primary_url,
        media_type,
        target_ext,
        audio_url,
        fallback_audio_url,
        referer,
        ua,
        cookie_header,
        target_path,
    );
    if primary.is_ok() {
        return primary;
    }
    for (alt_url, alt_audio) in fallback_candidates {
        let _ = fs::remove_file(target_path);
        let alt_fb_audio = alt_audio.as_deref().or(fallback_audio_url);
        if let Ok(outcome) = fetch_media_to_path(
            alt_url,
            media_type,
            target_ext,
            alt_audio.as_deref(),
            alt_fb_audio,
            referer,
            ua,
            cookie_header,
            target_path,
        ) {
            return Ok(outcome);
        }
    }
    primary
}

fn fetch_audio_only_to_path(
    video_url: Option<&str>,
    audio_url: Option<&str>,
    music_url: Option<&str>,
    referer: &str,
    ua: &str,
    cookie_header: Option<&str>,
    target_path: &Path,
) -> Result<(), String> {
    let ffmpeg = find_ffmpeg_binary()
        .ok_or_else(|| "FFmpeg is required to extract Audio Only (.m4a) files.".to_string())?;

    for candidate in [audio_url, music_url].into_iter().flatten() {
        let clean = candidate.trim();
        if !is_safe_http_url(clean) {
            continue;
        }
        let temp_a = unique_temp_media_path("audio-src", "bin");
        if curl_download_file(clean, referer, ua, cookie_header, &temp_a).is_ok() {
            let ext_res = extract_audio_aac_m4a_ffmpeg(&ffmpeg, &temp_a, target_path);
            let _ = fs::remove_file(&temp_a);
            if ext_res.is_ok() {
                validate_or_remove_mp4(target_path)?;
                return Ok(());
            }
        } else {
            let _ = fs::remove_file(&temp_a);
        }
    }

    let Some(v_url) = video_url.map(str::trim).filter(|s| is_safe_http_url(s)) else {
        return Err("No downloadable audio or video source found for .m4a extraction.".to_string());
    };
    let temp_v = unique_temp_media_path("audio-vid", "bin");
    if v_url.contains(".m3u8") {
        download_hls_to_mp4_ffmpeg(&ffmpeg, v_url, &temp_v)?;
    } else {
        curl_download_file(v_url, referer, ua, cookie_header, &temp_v)?;
    }
    validate_or_remove_video_input(&temp_v, true)?;
    let streams = probe_media_streams(&ffmpeg, &temp_v);
    if streams.audio_state == Mp4AudioState::Missing {
        let _ = fs::remove_file(&temp_v);
        return Err("This TikTok video stream has no audio track to extract.".to_string());
    }
    let res =
        extract_audio_aac_m4a_ffmpeg_with_streams(&ffmpeg, &temp_v, target_path, Some(&streams));
    let _ = fs::remove_file(&temp_v);
    res?;
    validate_or_remove_mp4(target_path)?;
    Ok(())
}

/// Download a direct media URL (video, image, or gif) into `Downloads/TikTok-Now/` with byte validation.
#[allow(clippy::too_many_arguments)]
pub async fn download_media_url(
    app: &AppHandle,
    url: String,
    media_type: String,
    referer: Option<String>,
    subfolder: Option<String>,
    file_stem: Option<String>,
    cookie_header: Option<String>,
    user_agent: Option<String>,
    audio_url: Option<String>,
) -> Result<String, String> {
    if !is_safe_http_url(&url) {
        return Err("TikTok did not provide a downloadable HTTP(S) media URL.".to_string());
    }
    let ext = media_extension(&media_type);
    let target_path = target_path_in_downloads(
        app,
        ext,
        subfolder.as_deref(),
        file_stem.as_deref(),
        false,
    )?;
    let clean_referer = referer
        .filter(|value| is_safe_http_url(value))
        .unwrap_or_else(|| "https://www.tiktok.com/".to_string());
    let ua = user_agent
        .map(|s| sanitize_header_value(&s))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_UA.to_string());

    tokio::task::spawn_blocking(move || -> Result<String, String> {
        let (final_path, _) = fetch_media_to_path(
            &url,
            &media_type,
            ext,
            audio_url.as_deref(),
            None,
            &clean_referer,
            &ua,
            cookie_header.as_deref(),
            &target_path,
        )?;
        Ok(final_path.to_string_lossy().into_owned())
    })
    .await
    .map_err(|error| error.to_string())?
}

/// Save raw bytes (video/image/text/json) into `Downloads/TikTok-Now/` with atomic write and byte validation.
pub async fn save_media_bytes(
    app: &AppHandle,
    data: Vec<u8>,
    media_type: String,
    subfolder: Option<String>,
    file_stem: Option<String>,
) -> Result<String, String> {
    if data.is_empty() {
        return Err("TikTok returned an empty media response.".to_string());
    }
    let ext = media_extension(&media_type);
    let allow_overwrite =
        subfolder.is_some() && matches!(ext, "txt" | "json");
    let target_path = target_path_in_downloads(
        app,
        ext,
        subfolder.as_deref(),
        file_stem.as_deref(),
        allow_overwrite,
    )?;

    tokio::task::spawn_blocking(move || -> Result<String, String> {
        if media_type.eq_ignore_ascii_case("video") {
            let ffmpeg = find_ffmpeg_binary();
            if !is_valid_download_video_header(&data, ffmpeg.is_some()) {
                return Err(
                    "The downloaded video was only a media fragment or invalid stream, not a complete MP4."
                        .to_string(),
                );
            }
            let temp_path = unique_temp_media_path_near(&target_path, "vbytes", "bin");
            if let Err(err) = fs::write(&temp_path, &data) {
                let _ = fs::remove_file(&temp_path);
                return Err(err.to_string());
            }
            let res = if let Some(ref ff) = ffmpeg {
                normalize_video_mp4(ff, &temp_path, None, &target_path).map(|_| ())
            } else {
                replace_file_atomically(&temp_path, &target_path)
            };
            let _ = fs::remove_file(&temp_path);
            res?;
            validate_or_remove_mp4(&target_path)?;
            return Ok(target_path.to_string_lossy().into_owned());
        }
        if media_type.eq_ignore_ascii_case("image") {
            if !is_valid_image_header(&data) {
                return Err(
                    "TikTok returned a non-image response (expired link or HTML error page)."
                        .to_string(),
                );
            }
            let temp_path = unique_temp_media_path_near(&target_path, "ibytes", "tmp");
            if let Err(err) = fs::write(&temp_path, &data) {
                let _ = fs::remove_file(&temp_path);
                return Err(err.to_string());
            }
            let res = normalize_image_file(find_ffmpeg_binary().as_deref(), &temp_path, &target_path);
            let _ = fs::remove_file(&temp_path);
            let final_path = res?;
            return Ok(final_path.to_string_lossy().into_owned());
        }
        let temp_path = unique_temp_media_path_near(&target_path, "bytes", ext);
        if let Err(err) = fs::write(&temp_path, &data) {
            let _ = fs::remove_file(&temp_path);
            return Err(err.to_string());
        }
        replace_file_atomically(&temp_path, &target_path)?;
        Ok(target_path.to_string_lossy().into_owned())
    })
    .await
    .map_err(|error| error.to_string())?
}

/// Save a formatted `caption.txt` / `post.txt` document for an inspected post.
pub async fn save_caption_file(
    app: &AppHandle,
    post: InspectedPost,
    custom_caption: Option<String>,
    subfolder: Option<String>,
    file_stem: Option<String>,
) -> Result<String, String> {
    let mut final_post = post;
    if let Some(cap) = custom_caption {
        final_post.caption = cap;
    }
    let stem = file_stem.unwrap_or_else(|| {
        build_source_filename_stem(
            &final_post.username,
            &final_post.created_at,
            &final_post.video_id,
        )
    });
    let doc = format_caption_document(&final_post);
    save_media_bytes(
        app,
        doc.into_bytes(),
        "text".to_string(),
        subfolder,
        Some(stem),
    )
    .await
}

/// Execute a user selection from the Downloader window (`single`, `audio`, `gif`, `caption`/`text`, or `bundle`).
#[allow(clippy::too_many_arguments)]
pub async fn download_post_selection(
    app: &AppHandle,
    post: InspectedPost,
    item_index: usize,
    format_id: Option<String>,
    mode: String,
    custom_caption: Option<String>,
    cookie_header: Option<String>,
    downloader_window_label: Option<&str>,
) -> Result<DownloadOutcome, String> {
    let mut final_post = post;
    if let Some(cap) = custom_caption {
        final_post.caption = cap;
    }
    if mode.eq_ignore_ascii_case("caption") || mode.eq_ignore_ascii_case("text") {
        let cap_path = save_caption_file(app, final_post, None, None, None).await?;
        return Ok(DownloadOutcome {
            saved_paths: Vec::new(),
            caption_path: Some(cap_path),
            metadata_path: None,
            subfolder: None,
            warning: None,
        });
    }

    if final_post.items.is_empty() && !mode.eq_ignore_ascii_case("audio") {
        return Err("No downloadable video or image items found for this post.".to_string());
    }

    let referer = if is_safe_http_url(&final_post.post_url) {
        final_post.post_url.clone()
    } else {
        "https://www.tiktok.com/".to_string()
    };
    let ua = DEFAULT_UA.to_string();

    if mode.eq_ignore_ascii_case("audio") {
        let item_opt = final_post
            .items
            .get(item_index)
            .or_else(|| final_post.items.first());
        let chosen_opt = item_opt.and_then(|item| {
            format_id
                .as_deref()
                .and_then(|id| item.formats.iter().find(|f| f.id == id))
                .or_else(|| item.formats.iter().find(|f| f.ext != "gif"))
                .or_else(|| item.formats.first())
        });
        let video_url = item_opt
            .filter(|i| i.kind == "video")
            .and_then(|_| chosen_opt.map(|c| c.video_url.clone()));
        let audio_candidate = chosen_opt
            .and_then(|c| c.audio_url.clone())
            .or_else(|| item_opt.and_then(|i| i.formats.iter().find_map(|f| f.audio_url.clone())));
        let music_url = final_post.music_url.clone();
        if video_url.is_none() && audio_candidate.is_none() && music_url.is_none() {
            return Err("No audio or video stream found for .m4a extraction.".to_string());
        }
        let base_stem = build_source_filename_stem(
            &final_post.username,
            &final_post.created_at,
            &final_post.video_id,
        );
        let stem = match item_opt {
            Some(it) => item_slide_stem(&base_stem, it.index, final_post.items.len()),
            None => base_stem,
        };
        let target_path = target_path_in_downloads(app, "m4a", None, Some(&stem), false)?;
        let target_str = target_path.to_string_lossy().into_owned();
        tokio::task::spawn_blocking(move || {
            fetch_audio_only_to_path(
                video_url.as_deref(),
                audio_candidate.as_deref(),
                music_url.as_deref(),
                &referer,
                &ua,
                cookie_header.as_deref(),
                &target_path,
            )
        })
        .await
        .map_err(|e| e.to_string())??;

        return Ok(DownloadOutcome {
            saved_paths: vec![target_str],
            caption_path: None,
            metadata_path: None,
            subfolder: None,
            warning: None,
        });
    }

    if mode.eq_ignore_ascii_case("gif") {
        let item = final_post
            .items
            .get(item_index)
            .or_else(|| final_post.items.first())
            .ok_or_else(|| "Selected media item not found.".to_string())?;
        if item.kind != "video" && item.kind != "gif" {
            return Err("Animated GIF export is only available for video items.".to_string());
        }
        let chosen = format_id
            .as_deref()
            .and_then(|id| item.formats.iter().find(|f| f.id == id))
            .or_else(|| item.formats.iter().find(|f| f.ext != "gif"))
            .or_else(|| item.formats.first())
            .ok_or_else(|| "No downloadable video format available for GIF export.".to_string())?;
        let base_stem = build_source_filename_stem(
            &final_post.username,
            &final_post.created_at,
            &final_post.video_id,
        );
        let stem = item_slide_stem(&base_stem, item.index, final_post.items.len());
        let target_path = target_path_in_downloads(app, "gif", None, Some(&stem), false)?;
        let video_url = chosen.video_url.clone();
        let fallback_candidates = collect_item_fallback_candidates(item, &video_url);
        let (final_path, warning) = tokio::task::spawn_blocking(move || {
            fetch_media_with_fallbacks(
                &video_url,
                &fallback_candidates,
                "gif",
                "gif",
                None,
                None,
                &referer,
                &ua,
                cookie_header.as_deref(),
                &target_path,
            )
        })
        .await
        .map_err(|e| e.to_string())??;

        return Ok(DownloadOutcome {
            saved_paths: vec![final_path.to_string_lossy().into_owned()],
            caption_path: None,
            metadata_path: None,
            subfolder: None,
            warning,
        });
    }

    if mode.eq_ignore_ascii_case("single") {
        let item = final_post
            .items
            .get(item_index)
            .or_else(|| final_post.items.first())
            .ok_or_else(|| "Selected media item not found.".to_string())?;
        let chosen = format_id
            .as_deref()
            .and_then(|id| item.formats.iter().find(|f| f.id == id))
            .or_else(|| item.formats.first())
            .ok_or_else(|| "No downloadable format available for this item.".to_string())?;

        let base_stem = build_source_filename_stem(
            &final_post.username,
            &final_post.created_at,
            &final_post.video_id,
        );
        let stem = item_slide_stem(&base_stem, item.index, final_post.items.len());
        let effective_kind = if chosen.ext == "gif" {
            "gif".to_string()
        } else {
            item.kind.clone()
        };
        let target_ext = if effective_kind == "gif" {
            "gif"
        } else if effective_kind == "video" {
            "mp4"
        } else {
            &chosen.ext
        };
        let target_path = target_path_in_downloads(app, target_ext, None, Some(&stem), false)?;
        let video_url = chosen.video_url.clone();
        let audio_url = chosen.audio_url.clone();
        let fallback_audio = select_fallback_audio(
            &item.formats,
            audio_url.as_deref(),
            final_post.music_url.as_deref(),
        );
        let fallback_candidates = collect_item_fallback_candidates(item, &video_url);
        let ext_owned = target_ext.to_string();
        let (final_path, warning) = tokio::task::spawn_blocking(move || {
            fetch_media_with_fallbacks(
                &video_url,
                &fallback_candidates,
                &effective_kind,
                &ext_owned,
                audio_url.as_deref(),
                fallback_audio.as_deref(),
                &referer,
                &ua,
                cookie_header.as_deref(),
                &target_path,
            )
        })
        .await
        .map_err(|e| e.to_string())??;

        return Ok(DownloadOutcome {
            saved_paths: vec![final_path.to_string_lossy().into_owned()],
            caption_path: None,
            metadata_path: None,
            subfolder: None,
            warning,
        });
    }

    // Full post bundle mode: Downloads/TikTok-Now/<stem>/ with media + caption.txt + metadata.json.
    let post_stem = build_source_filename_stem(
        &final_post.username,
        &final_post.created_at,
        &final_post.video_id,
    );
    let subfolder = post_stem.clone();
    type BundleItemResult = (usize, Result<(PathBuf, Option<String>), String>);
    let item_count = final_post.items.len();
    let mut indexed_results: Vec<BundleItemResult> = Vec::with_capacity(item_count);
    let mut join_set = tokio::task::JoinSet::new();
    let emit_progress = |completed: usize| {
        if let Some(label) = downloader_window_label {
            if let Some(dl_win) = app.get_webview_window(label) {
                let _ = dl_win.emit(
                    "downloader-progress",
                    serde_json::json!({ "completed": completed, "total": item_count }),
                );
            }
        }
    };

    for (idx, item) in final_post.items.iter().enumerate() {
        let chosen = if idx == item_index {
            format_id
                .as_deref()
                .and_then(|id| item.formats.iter().find(|f| f.id == id))
                .or_else(|| item.formats.first())
        } else {
            item.formats.iter().find(|f| f.ext != "gif").or_else(|| item.formats.first())
        };
        let Some(fmt) = chosen else {
            continue;
        };
        let stem = item_slide_stem(&post_stem, idx, item_count);
        let effective_kind = if fmt.ext == "gif" {
            "gif".to_string()
        } else {
            item.kind.clone()
        };
        let target_ext = if effective_kind == "gif" {
            "gif"
        } else if effective_kind == "video" {
            "mp4"
        } else {
            &fmt.ext
        };
        let target_path =
            target_path_in_downloads(app, target_ext, Some(&subfolder), Some(&stem), false)?;
        let v_url = fmt.video_url.clone();
        let a_url = fmt.audio_url.clone();
        let fb_audio = select_fallback_audio(
            &item.formats,
            a_url.as_deref(),
            final_post.music_url.as_deref(),
        );
        let fallback_candidates = collect_item_fallback_candidates(item, &v_url);
        let ext_owned = target_ext.to_string();
        let ref_clone = referer.clone();
        let ua_clone = ua.clone();
        let cookie_clone = cookie_header.clone();

        while join_set.len() >= 6 {
            if let Some(joined) = join_set.join_next().await {
                indexed_results.push(joined.map_err(|e| e.to_string())?);
                emit_progress(indexed_results.len());
            }
        }

        join_set.spawn_blocking(move || {
            let res = fetch_media_with_fallbacks(
                &v_url,
                &fallback_candidates,
                &effective_kind,
                &ext_owned,
                a_url.as_deref(),
                fb_audio.as_deref(),
                &ref_clone,
                &ua_clone,
                cookie_clone.as_deref(),
                &target_path,
            );
            (idx, res)
        });
    }

    while let Some(joined) = join_set.join_next().await {
        indexed_results.push(joined.map_err(|e| e.to_string())?);
        emit_progress(indexed_results.len());
    }

    indexed_results.sort_by_key(|(idx, _)| *idx);
    let mut saved_paths = Vec::new();
    let mut first_warning = None;
    let mut failed_slides = Vec::new();
    for (idx, res) in indexed_results {
        match res {
            Ok((final_path, warn_opt)) => {
                saved_paths.push(final_path.to_string_lossy().into_owned());
                if first_warning.is_none() {
                    first_warning = warn_opt;
                }
            }
            Err(err) => {
                failed_slides.push((idx + 1).to_string());
                if first_warning.is_none() {
                    first_warning = Some(err);
                }
            }
        }
    }
    if !failed_slides.is_empty() {
        let slide_list = failed_slides.join(", ");
        let detail = first_warning.take().unwrap_or_default();
        first_warning = Some(if detail.is_empty() {
            format!("Items [{slide_list}] could not be downloaded.")
        } else {
            format!("Items [{slide_list}] failed ({detail}).")
        });
    }

    let caption_path = save_caption_file(
        app,
        final_post.clone(),
        None,
        Some(subfolder.clone()),
        Some("caption".to_string()),
    )
    .await
    .ok();

    let metadata_path = match build_metadata_json(&final_post) {
        Ok(meta_json) => save_media_bytes(
            app,
            meta_json.into_bytes(),
            "json".to_string(),
            Some(subfolder.clone()),
            Some("metadata".to_string()),
        )
        .await
        .ok(),
        Err(_) => None,
    };

    if saved_paths.is_empty() && caption_path.is_none() {
        return Err(first_warning.unwrap_or_else(|| "Post download failed.".to_string()));
    }

    Ok(DownloadOutcome {
        saved_paths,
        caption_path,
        metadata_path,
        subfolder: Some(subfolder),
        warning: first_warning,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tiktok_video_photo_shortlink_and_rejects_foreign_hosts() {
        assert_eq!(
            video_id_from_url("https://www.tiktok.com/@creator/video/7345678901234567890?is_from_webapp=1"),
            Some("7345678901234567890".to_string())
        );
        assert_eq!(
            video_id_from_url("https://www.tiktok.com/@creator/photo/7345678901234567891"),
            Some("7345678901234567891".to_string())
        );
        assert_eq!(
            video_id_from_url("https://m.tiktok.com/v/7345678901234567892.html"),
            Some("7345678901234567892".to_string())
        );
        assert_eq!(
            video_id_from_url("https://www.tiktok.com/foryou?item_id=7345678901234567893"),
            Some("7345678901234567893".to_string())
        );
        assert_eq!(
            video_id_from_url("7345678901234567894"),
            Some("7345678901234567894".to_string())
        );
        assert_eq!(
            video_id_from_url("https://evil.example/@creator/video/7345678901234567890"),
            None
        );
        assert_eq!(
            username_from_url("https://www.tiktok.com/@tokyo_nights/video/7345678901234567890"),
            Some("tokyo_nights".to_string())
        );
        assert!(is_tiktok_shortlink("https://vm.tiktok.com/ZM8abc123/"));
        assert!(is_tiktok_shortlink("https://vt.tiktok.com/ZS8abc123/"));
        assert!(is_tiktok_shortlink("https://www.tiktok.com/t/ZT8abc123/"));
        assert!(!is_tiktok_shortlink("https://www.tiktok.com/@user/video/12345"));
    }

    #[test]
    fn enforces_tiktok_host_boundaries_and_userinfo_bypass_protection() {
        assert!(is_tiktok_app_host("https://www.tiktok.com/@user/video/123"));
        assert!(is_tiktok_app_host("https://vm.tiktok.com/ZM123/"));
        assert!(!is_tiktok_app_host("https://p16-sign-sg.tiktokcdn.com/a.mp4"));
        assert!(!is_tiktok_app_host("https://eviltiktok.com/"));
        assert!(!is_tiktok_app_host("https://tiktok.com@evil.example/"));

        assert!(is_tiktok_media_url("https://v16-webapp-prime.tiktok.com/video/tos/clip.mp4"));
        assert!(is_tiktok_media_url("https://p16-sign-va.tiktokcdn.com/tos-maliva/img.webp"));
        assert!(is_tiktok_media_url("https://v19.tiktokcdn-us.com/clip.mp4"));
        assert!(!is_tiktok_media_url("http://v16-webapp-prime.tiktok.com/clip.mp4"));
        assert!(!is_tiktok_media_url("https://tiktokcdn.com@evil.example/clip.mp4"));
        assert!(!is_tiktok_media_url("https://evil.example/clip.mp4"));
    }

    #[test]
    fn strips_mse_range_params_and_parses_dash_manifest() {
        let stripped = strip_media_range_params(
            "https://v16-webapp-prime.tiktok.com/v/clip.mp4?x-expires=999&amp;bytestart=0&amp;byteend=512000&range=0-100",
        );
        assert_eq!(
            stripped,
            "https://v16-webapp-prime.tiktok.com/v/clip.mp4?x-expires=999"
        );

        let xml = r#"<MPD><Period>
          <AdaptationSet mimeType="video/mp4">
            <Representation codecs="av01.0.08M.08" bandwidth="3800000" width="1080" height="1920"><BaseURL>https://v16-webapp-prime.tiktok.com/v/av1.mp4</BaseURL></Representation>
            <Representation codecs="avc1.64002a" bandwidth="2200000" width="1080" height="1920"><BaseURL>https://v16-webapp-prime.tiktok.com/v/h264.mp4?bytestart=0&amp;byteend=99</BaseURL></Representation>
          </AdaptationSet>
          <AdaptationSet mimeType="audio/mp4">
            <Representation codecs="opus" bandwidth="192000"><BaseURL>https://v16-webapp-prime.tiktok.com/v/a.opus</BaseURL></Representation>
            <Representation codecs="mp4a.40.2" bandwidth="128000"><BaseURL>https://v16-webapp-prime.tiktok.com/v/a.m4a</BaseURL></Representation>
          </AdaptationSet>
        </Period></MPD>"#;
        let (videos, audios) = parse_dash_manifest(xml);
        assert_eq!(videos.len(), 2);
        assert_eq!(videos[0].url, "https://v16-webapp-prime.tiktok.com/v/h264.mp4");
        assert_eq!(audios[0].url, "https://v16-webapp-prime.tiktok.com/v/a.m4a");
    }

    #[test]
    fn extracts_universal_rehydration_video_sorting_h264_and_music() {
        let html = r#"<html><body><script id="__UNIVERSAL_DATA_FOR_REHYDRATION__" type="application/json">{
          "__DEFAULT_SCOPE__": {
            "webapp.video-detail": {
              "itemInfo": {
                "itemStruct": {
                  "id": "7345000111222333444",
                  "desc": "Shinjuku rain #tokyo",
                  "createTime": "1731700000",
                  "author": { "uniqueId": "tokyo_walker", "nickname": "Tokyo Walker" },
                  "music": {
                    "title": "Rainy Loop",
                    "authorName": "Lofi Artist",
                    "playUrl": "https://sf16-ies-music-va.tiktokcdn.com/obj/music/rain.mp3"
                  },
                  "video": {
                    "width": 1080,
                    "height": 1920,
                    "originCover": "https://p16-sign-sg.tiktokcdn.com/cover.jpg",
                    "bitrateInfo": [
                      {
                        "Bitrate": 2400000,
                        "CodecType": "h265_hvc1",
                        "PlayAddr": { "Width": 1080, "Height": 1920, "UrlList": ["https://v16-webapp-prime.tiktok.com/v/hevc1080.mp4"] }
                      },
                      {
                        "Bitrate": 1800000,
                        "CodecType": "h264",
                        "PlayAddr": { "Width": 1080, "Height": 1920, "UrlList": ["https://v16-webapp-prime.tiktok.com/v/h264_1080.mp4", "https://v19-webapp-prime.tiktok.com/v/h264_1080_mirror.mp4"] }
                      },
                      {
                        "Bitrate": 900000,
                        "CodecType": "h264",
                        "PlayAddr": { "Width": 720, "Height": 1280, "UrlList": ["https://v16-webapp-prime.tiktok.com/v/h264_720.mp4"] }
                      }
                    ]
                  }
                }
              }
            }
          }
        }</script></body></html>"#;

        let post = extract_post_from_html(html, Some("7345000111222333444"), true)
            .expect("parsed rehydration");
        assert_eq!(post.username, "tokyo_walker");
        assert_eq!(post.display_name, "Tokyo Walker");
        assert_eq!(post.caption, "Shinjuku rain #tokyo");
        assert_eq!(post.music_title.as_deref(), Some("Rainy Loop"));
        assert_eq!(post.items.len(), 1);
        assert_eq!(post.items[0].kind, "video");
        assert_eq!(post.items[0].formats[0].codecs, "avc1");
        assert_eq!(
            post.items[0].formats[0].video_url,
            "https://v16-webapp-prime.tiktok.com/v/h264_1080.mp4"
        );
        assert_eq!(
            post.items[0].formats[0].fallback_urls,
            vec!["https://v19-webapp-prime.tiktok.com/v/h264_1080_mirror.mp4"]
        );
        assert!(post.items[0].formats.iter().any(|f| f.ext == "gif"));
    }

    #[test]
    fn extracts_sigi_state_photo_slideshow_with_fallback_urls() {
        let html = r#"<html><body><script id="SIGI_STATE" type="application/json">{
          "ItemModule": {
            "7355000111222333444": {
              "id": "7355000111222333444",
              "desc": "Three photo cards",
              "createTime": 1731800000,
              "author": { "uniqueId": "photo_story", "nickname": "Photo Story" },
              "imagePost": {
                "images": [
                  {
                    "imageWidth": 1080,
                    "imageHeight": 1440,
                    "imageURL": { "urlList": ["https://p16-sign-sg.tiktokcdn.com/slide1.webp", "https://p19-sign-sg.tiktokcdn.com/slide1.webp"] }
                  },
                  {
                    "imageWidth": 1080,
                    "imageHeight": 1440,
                    "imageURL": { "urlList": ["https://p16-sign-sg.tiktokcdn.com/slide2.jpg"] }
                  }
                ]
              }
            }
          }
        }</script></body></html>"#;

        let post = extract_post_from_html(html, Some("7355000111222333444"), false)
            .expect("parsed sigi photo");
        assert_eq!(post.items.len(), 2);
        assert_eq!(post.items[0].kind, "image");
        assert_eq!(post.items[0].formats[0].width, 1080);
        assert_eq!(post.items[0].formats[0].height, 1440);
        assert_eq!(post.items[0].formats[0].ext, "webp");
        assert_eq!(
            post.items[0].formats[0].fallback_urls,
            vec!["https://p19-sign-sg.tiktokcdn.com/slide1.webp"]
        );
        assert!(post.post_url.contains("/photo/7355000111222333444"));
    }

    #[test]
    fn oembed_and_merge_inspected_post_combine_sparse_http_and_rich_seed() {
        let sparse_og = extract_post_from_html(
            r#"<html><head><meta property="og:video" content="https://v16-webapp-prime.tiktok.com/v/clip.mp4" /></head></html>"#,
            Some("73990011"),
            true,
        )
        .expect("og fallback");
        let oembed = extract_oembed_metadata(
            r#"{"title":"OEmbed Caption","author_name":"OEmbed Name","author_unique_id":"oembed_user","embed_product_id":"73990011"}"#,
            Some("73990011"),
        )
        .expect("oembed");
        let merged = merge_inspected_post(sparse_og, oembed);
        assert_eq!(merged.username, "oembed_user");
        assert_eq!(merged.display_name, "OEmbed Name");
        assert_eq!(merged.caption, "OEmbed Caption");
        assert_eq!(merged.items.len(), 1);

        let seed_json = serde_json::json!({
            "ttnowSeed": true,
            "videoId": "73990011",
            "username": "seed_user",
            "items": [
                { "kind": "image", "width": 1080, "height": 1440, "urls": ["https://p16-sign-sg.tiktokcdn.com/s1.jpg"] },
                { "kind": "image", "width": 1080, "height": 1440, "urls": ["https://p16-sign-sg.tiktokcdn.com/s2.jpg"] }
            ]
        })
        .to_string();
        let seed_post =
            extract_post_from_json_text(&seed_json, Some("73990011"), true).expect("seed");
        let picked = pick_richer_inspected_post(merged, seed_post);
        assert_eq!(picked.items.len(), 2);
        assert_eq!(picked.caption, "OEmbed Caption");
    }

    #[test]
    fn sanitizes_windows_reserved_stems_and_escapes_curl_stdin_headers() {
        assert!(is_windows_reserved_stem("CON"));
        assert!(is_windows_reserved_stem("lpt9.txt"));
        assert!(!is_windows_reserved_stem("content"));
        assert_eq!(
            build_source_filename_stem("CON", "2026-11-27T00:00:00Z", "7345678"),
            "_CON-20261127-7345678"
        );

        let raw = build_curl_stdin_headers(Some("sessionid=\"a\\b\"\r\nurl = https://evil.test"));
        let text = String::from_utf8(raw).expect("utf8");
        assert!(text.contains("header = \"Cookie: sessionid=\\\"a\\\\b\\\"url = https://evil.test\""));
        assert_eq!(text.lines().count(), 1);
    }

    #[test]
    fn validates_magic_bytes_rejects_html_and_detects_mp4_layout() {
        assert!(looks_like_html_error(b"  <!DOCTYPE html><html>403</html>"));
        assert!(!is_valid_download_video_header(b"<html>denied</html>", true));
        assert!(is_valid_download_video_header(
            b"\x00\x00\x00\x18ftypisom",
            false
        ));
        assert!(!is_valid_download_video_header(
            &[0x1a, 0x45, 0xdf, 0xa3, 0x9f],
            false
        ));
        assert!(is_valid_download_video_header(
            &[0x1a, 0x45, 0xdf, 0xa3, 0x9f],
            true
        ));

        assert_eq!(
            detect_image_format(b"\xFF\xD8\xFF\xE0\x00\x10JFIF").map(|f| f.kind),
            Some(ImageFormatKind::Jpg)
        );
        assert_eq!(
            detect_image_format(b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0d").map(|f| f.kind),
            Some(ImageFormatKind::Png)
        );
        assert_eq!(
            detect_image_format(b"RIFF\x24\x00\x00\x00WEBPVP8L").map(|f| (f.kind, f.is_lossless_or_alpha)),
            Some((ImageFormatKind::Webp, true))
        );
        assert_eq!(
            detect_image_format(b"\x00\x00\x00\x18ftypavif\x00\x00\x00\x00mif1").map(|f| f.kind),
            Some(ImageFormatKind::Avif)
        );
        assert_eq!(
            detect_image_format(b"\x00\x00\x00\x18ftypheic\x00\x00\x00\x00mif1").map(|f| f.kind),
            Some(ImageFormatKind::Heic)
        );
        assert!(detect_image_format(b"\x00\x00\x00\x18ftypisom\x00\x00\x02\x00isom").is_none());
        assert!(detect_image_format(b"<!doctype html>").is_none());

        let fast = unique_temp_media_path("tt-layout-fast", "mp4");
        let mut fast_bytes = Vec::new();
        fast_bytes.extend_from_slice(&20u32.to_be_bytes());
        fast_bytes.extend_from_slice(b"ftypisom\x00\x00\x02\x00isom");
        fast_bytes.extend_from_slice(&8u32.to_be_bytes());
        fast_bytes.extend_from_slice(b"moov");
        fast_bytes.extend_from_slice(&8u32.to_be_bytes());
        fast_bytes.extend_from_slice(b"mdat");
        fs::write(&fast, &fast_bytes).unwrap();
        let layout = inspect_mp4_layout(&fast);
        let _ = fs::remove_file(&fast);
        assert!(layout.is_mp4 && layout.is_faststart && !layout.is_fragmented);
    }

    #[test]
    fn probe_state_and_cancel_guard_and_atomic_replace_work() {
        let state = DownloaderProbeState::default();
        let (tx1, mut rx1) = oneshot::channel();
        state.register_probe("req-1".to_string(), tx1);
        assert!(!state.resolve_probe("wrong", "nope".to_string()));
        assert!(state.resolve_probe("req-1", "{\"ok\":true}".to_string()));
        assert!(!state.resolve_probe("req-1", "dup".to_string()));
        assert_eq!(rx1.try_recv().ok().as_deref(), Some("{\"ok\":true}"));

        let (tx2, _rx2) = oneshot::channel();
        state.register_probe("req-guard".to_string(), tx2);
        {
            let _guard = ProbeCancelGuard::new(&state, "req-guard".to_string());
        }
        assert!(!state.resolve_probe("req-guard", "late".to_string()));

        let target = unique_temp_media_path("tt-atom-target", "mp4");
        let temp = unique_temp_media_path_near(&target, "tt-atom-src", "mp4");
        fs::write(&target, b"old").unwrap();
        fs::write(&temp, b"new").unwrap();
        replace_file_atomically(&temp, &target).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"new");
        assert!(!temp.exists());
        let _ = fs::remove_file(&target);
    }

    #[test]
    fn ffmpeg_normalizes_video_extracts_m4a_exports_gif_and_converts_webp() {
        let Some(ffmpeg) = find_ffmpeg_binary() else {
            return;
        };
        let native = unique_temp_media_path("tt-native", "mp4");
        let native_out = unique_temp_media_path("tt-native-out", "mp4");
        let m4a_out = unique_temp_media_path("tt-audio", "m4a");
        let gif_out = unique_temp_media_path("tt-anim", "gif");

        let mut c1 = Command::new(&ffmpeg);
        apply_no_window(&mut c1);
        let s1 = c1
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=blue:s=64x64:d=0.3",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=0.3",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
                "-movflags",
                "+faststart",
            ])
            .arg(&native)
            .status()
            .expect("spawn ffmpeg");
        assert!(s1.success());

        normalize_video_mp4(&ffmpeg, &native, None, &native_out).expect("passthrough");
        assert_eq!(fs::read(&native).unwrap(), fs::read(&native_out).unwrap());

        extract_audio_aac_m4a_ffmpeg(&ffmpeg, &native, &m4a_out).expect("extract m4a");
        let m4a_streams = probe_media_streams(&ffmpeg, &m4a_out);
        assert!(m4a_streams.video_codec.is_empty());
        assert_eq!(m4a_streams.audio_state, Mp4AudioState::Aac);

        convert_mp4_to_gif_ffmpeg(&ffmpeg, &native, &gif_out).expect("export gif");
        assert_eq!(
            detect_image_format(&fs::read(&gif_out).unwrap()).map(|f| f.kind),
            Some(ImageFormatKind::Gif)
        );

        for p in [native, native_out, m4a_out, gif_out] {
            let _ = fs::remove_file(p);
        }
    }

    #[test]
    fn muxes_video_only_mp4_with_music_url_fallback() {
        let formats = vec![MediaFormat {
            id: "v-only".to_string(),
            label: "720p MP4 HEVC".to_string(),
            width: 720,
            height: 1280,
            bandwidth: 11_000,
            codecs: "hvc1".to_string(),
            ext: "mp4".to_string(),
            requires_mux: false,
            video_url: "https://example.invalid/video.mp4".to_string(),
            audio_url: None,
            fallback_urls: Vec::new(),
        }];
        let picked = select_fallback_audio(
            &formats,
            None,
            Some("https://sf16-ies-music.tiktokcdn.com/obj/audio.mp3"),
        );
        assert_eq!(
            picked.as_deref(),
            Some("https://sf16-ies-music.tiktokcdn.com/obj/audio.mp3")
        );

        let Some(ffmpeg) = find_ffmpeg_binary() else {
            return;
        };
        let vid_only = unique_temp_media_path("tt-vidonly", "mp4");
        let aud_only = unique_temp_media_path("tt-audonly", "m4a");
        let muxed_out = unique_temp_media_path("tt-muxed", "mp4");

        let mut cv = Command::new(&ffmpeg);
        apply_no_window(&mut cv);
        assert!(cv
            .args([
                "-y", "-f", "lavfi", "-i", "color=c=black:s=64x64:d=0.3",
                "-an", "-c:v", "libx264", "-pix_fmt", "yuv420p", "-movflags", "+faststart",
            ])
            .arg(&vid_only)
            .status()
            .expect("spawn ffmpeg video")
            .success());

        let mut ca = Command::new(&ffmpeg);
        apply_no_window(&mut ca);
        assert!(ca
            .args([
                "-y", "-f", "lavfi", "-i", "sine=frequency=520:duration=0.3",
                "-vn", "-c:a", "aac", "-b:a", "128k",
            ])
            .arg(&aud_only)
            .status()
            .expect("spawn ffmpeg audio")
            .success());

        let pre = probe_media_streams(&ffmpeg, &vid_only);
        assert_eq!(pre.audio_state, Mp4AudioState::Missing);

        let warn = normalize_video_mp4_with_streams(
            &ffmpeg,
            &vid_only,
            Some(&aud_only),
            &muxed_out,
            Some(&pre),
        )
        .expect("mux fallback audio");
        assert!(warn.is_none());

        let out_streams = probe_media_streams(&ffmpeg, &muxed_out);
        assert_eq!(out_streams.video_codec, "h264");
        assert_eq!(out_streams.audio_state, Mp4AudioState::Aac);

        for p in [vid_only, aud_only, muxed_out] {
            let _ = fs::remove_file(p);
        }
    }
}
