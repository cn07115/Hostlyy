// WebDAV sync for cross-device hosts configuration.
// Stores the WebDAV URL + username in `config.local.json`, password in
// the system keychain (Windows Credential Manager / macOS Keychain / Linux
// Secret Service). Synced files live under `<base_url>/hostly/`:
//   <base_url>/hostly/config.sync.json
//   <base_url>/hostly/profiles/<uuid>.txt
//
// Conflict policy: last-write-wins, computed from HTTP Last-Modified headers
// when the server provides them. Local is authoritative when both sides are
// equal (so offline edits don't get clobbered when a sync runs after a
// network drop).

use keyring::Entry;
use minreq::Method;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use chrono::TimeZone;
use tauri::Manager;
use tokio::sync::Mutex;
use tokio::time::interval;

const KEYRING_SERVICE: &str = "hostly-webdav";
const REMOTE_DIR: &str = "hostly";
const PROFILES_REMOTE_DIR: &str = "hostly/profiles";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebDavConfig {
    pub url: String,
    pub username: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncStatus {
    pub configured: bool,
    pub last_sync: Option<String>,
    pub last_status: Option<String>,
    pub last_message: Option<String>,
    pub username: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncResult {
    pub uploaded: Vec<String>,
    pub downloaded: Vec<String>,
    pub deleted_remote: Vec<String>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl SyncResult {
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if !self.uploaded.is_empty() {
            parts.push(format!("上传 {} 个", self.uploaded.len()));
        }
        if !self.downloaded.is_empty() {
            parts.push(format!("下载 {} 个", self.downloaded.len()));
        }
        if !self.deleted_remote.is_empty() {
            parts.push(format!("远端删除 {} 个", self.deleted_remote.len()));
        }
        if self.errors.is_empty() {
            if parts.is_empty() {
                "无变化".to_string()
            } else {
                parts.join("，")
            }
        } else {
            format!("{}；错误 {} 个", parts.join("，"), self.errors.len())
        }
    }
}

fn keyring_entry(username: &str) -> Result<Entry, String> {
    Entry::new(KEYRING_SERVICE, username).map_err(|e| format!("无法访问系统 keychain: {}", e))
}

pub fn save_credentials(username: &str, password: &str) -> Result<(), String> {
    if username.is_empty() {
        return Err("WebDAV 用户名不能为空".to_string());
    }
    if password.is_empty() {
        // Allow clearing by deleting the entry
        delete_credentials(username);
        return Ok(());
    }
    let entry = keyring_entry(username)?;
    entry
        .set_password(password)
        .map_err(|e| format!("写入 keychain 失败: {}", e))?;
    Ok(())
}

pub fn delete_credentials(username: &str) {
    if username.is_empty() {
        return;
    }
    if let Ok(entry) = keyring_entry(username) {
        let _ = entry.delete_credential();
    }
}

pub fn load_credentials(username: &str) -> Result<String, String> {
    if username.is_empty() {
        return Err("WebDAV 用户名未配置".to_string());
    }
    let entry = keyring_entry(username)?;
    entry
        .get_password()
        .map_err(|e| format!("读取 keychain 失败: {}", e))
}

pub fn test_connection(config: &WebDavConfig, password: &str) -> Result<String, String> {
    let base = normalize_base(&config.url)?;
    let probe_url = format!("{}/", base);
    let response = dav_request(
        Method::Custom("PROPFIND".into()),
        &probe_url,
        &config.username,
        password,
        Some("0"),
        None,
    )?;
    let status = response.status_code;
    if (200..300).contains(&status) {
        Ok(format!("连接成功 (HTTP {})", status))
    } else {
        Err(format!("WebDAV 服务器返回 HTTP {}", status))
    }
}

pub fn perform_sync(
    app_dir: &PathBuf,
    config: &WebDavConfig,
    password: &str,
) -> Result<SyncResult, String> {
    let base = normalize_base(&config.url)?;
    let mut result = SyncResult {
        uploaded: Vec::new(),
        downloaded: Vec::new(),
        deleted_remote: Vec::new(),
        errors: Vec::new(),
        warnings: Vec::new(),
    };

    // 1. Ensure remote dir exists (MKCOL with parent)
    if let Err(e) = ensure_remote_dir(&format!("{}/{}", base, REMOTE_DIR), &config.username, password) {
        result.errors.push(format!("创建远端目录失败: {}", e));
        return Ok(result);
    }
    if let Err(e) = ensure_remote_dir(&format!("{}/{}", base, PROFILES_REMOTE_DIR), &config.username, password) {
        result.errors.push(format!("创建 profiles 目录失败: {}", e));
        // Continue: MKCOL is best-effort
    }

    // 2. Read local sync state
    let sync_path = app_dir.join("config.sync.json");
    let profiles_dir = app_dir.join("profiles");

    // 3. List remote files
    let remote_listing = list_remote(&format!("{}/", base), &config.username, password);

    // 4. Upload: local file is newer OR remote doesn't have it
    if sync_path.exists() {
        let local_mtime = file_mtime(&sync_path).unwrap_or(0);
        let remote_mtime_config = remote_listing
            .as_ref()
            .ok()
            .and_then(|list| list.get(&format!("{}/config.sync.json", base)).copied())
            .unwrap_or(0);

        if local_mtime > remote_mtime_config {
            // Staleness check: if our local change is way newer than remote,
            // we might be overwriting someone else's fresher work.
            if let Some(warn) = check_staleness(local_mtime, remote_mtime_config) {
                result.warnings.push(warn);
            }
            let content = std::fs::read_to_string(&sync_path).map_err(|e| format!("读取本地 sync 配置失败: {}", e))?;
            let url = format!("{}/config.sync.json", base);
            match dav_put(&url, &config.username, password, &content) {
                Ok(()) => result.uploaded.push("config.sync.json".to_string()),
                Err(e) => result.errors.push(format!("上传 config.sync.json 失败: {}", e)),
            }
        } else if remote_mtime_config > local_mtime {
            // Remote is newer, download
            let url = format!("{}/config.sync.json", base);
            match dav_get(&url, &config.username, password) {
                Ok(content) => {
                    std::fs::write(&sync_path, &content).map_err(|e| format!("写入本地 config.sync.json 失败: {}", e))?;
                    result.downloaded.push("config.sync.json".to_string());
                }
                Err(e) => result.errors.push(format!("下载 config.sync.json 失败: {}", e)),
            }
        }
    }

    // 5. Sync profiles
    let local_profiles = list_local_profiles(&profiles_dir);
    let remote_profiles = remote_listing
        .as_ref()
        .ok()
        .cloned()
        .unwrap_or_default();

    // 5a. Upload/download matching profiles
    for (id, local_mtime) in &local_profiles {
        let url = format!("{}/profiles/{}.txt", base, id);
        let remote_mtime = remote_profiles.get(&url).copied().unwrap_or(0);
        if *local_mtime > remote_mtime {
            if let Some(warn) = check_staleness(*local_mtime, remote_mtime) {
                result.warnings.push(format!("profiles/{}.txt: {}", id, warn));
            }
            let path = profiles_dir.join(format!("{}.txt", id));
            if path.exists() {
                let content = std::fs::read_to_string(&path).unwrap_or_default();
                match dav_put(&url, &config.username, password, &content) {
                    Ok(()) => result.uploaded.push(format!("profiles/{}.txt", id)),
                    Err(e) => result.errors.push(format!("上传 {}.txt 失败: {}", id, e)),
                }
            }
        } else if remote_mtime > *local_mtime {
            match dav_get(&url, &config.username, password) {
                Ok(content) => {
                    let path = profiles_dir.join(format!("{}.txt", id));
                    std::fs::write(&path, &content).map_err(|e| format!("写入本地 {}.txt 失败: {}", id, e)).ok();
                    result.downloaded.push(format!("profiles/{}.txt", id));
                }
                Err(e) => result.errors.push(format!("下载 {}.txt 失败: {}", id, e)),
            }
        }
    }

    // 5b. Download remote-only profiles
    for (url, remote_mtime) in &remote_profiles {
        if !url.contains("/profiles/") {
            continue; // Only handle profiles in this pass
        }
        let filename = url.rsplit('/').next().unwrap_or("");
        let id = filename.trim_end_matches(".txt");
        if local_profiles.contains_key(id) {
            continue; // Already handled above
        }
        match dav_get(url, &config.username, password) {
            Ok(content) => {
                let path = profiles_dir.join(filename);
                std::fs::write(&path, &content).ok();
                result.downloaded.push(format!("profiles/{}", filename));
            }
            Err(e) => result.errors.push(format!("下载 {} 失败: {}", filename, e)),
        }
        let _ = remote_mtime; // currently unused
    }

    // 5c. Delete remote profiles that don't exist locally
    for (url, _) in &remote_profiles {
        if !url.contains("/profiles/") {
            continue;
        }
        let filename = url.rsplit('/').next().unwrap_or("");
        let id = filename.trim_end_matches(".txt");
        if !local_profiles.contains_key(id) {
            match dav_delete(url, &config.username, password) {
                Ok(()) => result.deleted_remote.push(format!("profiles/{}", filename)),
                Err(e) => result.errors.push(format!("删除远端 {} 失败: {}", filename, e)),
            }
        }
    }

    Ok(result)
}

fn normalize_base(url: &str) -> Result<String, String> {
    let trimmed = url.trim_end_matches('/').to_string();
    if trimmed.is_empty() {
        return Err("WebDAV URL 不能为空".to_string());
    }
    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        return Err("WebDAV URL 必须以 http:// 或 https:// 开头".to_string());
    }
    Ok(trimmed.to_string())
}

fn dav_request(
    method: Method,
    url: &str,
    username: &str,
    password: &str,
    depth: Option<&str>,
    body: Option<&[u8]>,
) -> Result<minreq::Response, String> {
    let mut req = minreq::Request::new(method, url);
    req = req.with_header("User-Agent", "Hostly/1.2");
    if !username.is_empty() {
        let creds = format!("{}:{}", username, password);
        req = req.with_header("Authorization", format!("Basic {}", base64_encode(&creds)));
    }
    if let Some(d) = depth {
        req = req.with_header("Depth", d);
    }
    if let Some(b) = body {
        req = req.with_header("Content-Type", "application/xml; charset=utf-8");
        req = req.with_body(b);
    }
    req.send().map_err(|e| format!("HTTP 错误: {}", e))
}

fn dav_get(url: &str, username: &str, password: &str) -> Result<String, String> {
    let resp = dav_request(Method::Get, url, username, password, None, None)?;
    if resp.status_code != 200 {
        return Err(format!("HTTP {}", resp.status_code));
    }
    resp.as_str().map(|s| s.to_string()).map_err(|e| format!("解码失败: {}", e))
}

fn dav_put(url: &str, username: &str, password: &str, body: &str) -> Result<(), String> {
    let resp = dav_request(Method::Put, url, username, password, None, Some(body.as_bytes()))?;
    let status = resp.status_code;
    if !(200..300).contains(&status) {
        return Err(format!("HTTP {}", status));
    }
    Ok(())
}

fn dav_delete(url: &str, username: &str, password: &str) -> Result<(), String> {
    let resp = dav_request(Method::Delete, url, username, password, None, None)?;
    let status = resp.status_code;
    // 204 No Content or 200 OK both indicate success
    if !(200..300).contains(&status) {
        return Err(format!("HTTP {}", status));
    }
    Ok(())
}

fn ensure_remote_dir(url: &str, username: &str, password: &str) -> Result<(), String> {
    // Try MKCOL; if it returns 405 (method not allowed, dir already exists), that's fine.
    let resp = dav_request(Method::Custom("MKCOL".into()), url, username, password, None, None)?;
    let status = resp.status_code;
    if (200..300).contains(&status) || status == 405 {
        Ok(())
    } else {
        Err(format!("MKCOL failed: HTTP {}", status))
    }
}

fn list_remote(
    base: &str,
    username: &str,
    password: &str,
) -> Result<std::collections::HashMap<String, i64>, String> {
    // PROPFIND with Depth: 1 returns immediate children
    let body = b"<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
        <d:propfind xmlns:d=\"DAV:\">\n  \
        <d:prop><d:getlastmodified/></d:prop>\n\
        </d:propfind>";
    let resp = dav_request(
        Method::Custom("PROPFIND".into()),
        base,
        username,
        password,
        Some("1"),
        Some(body.as_slice()),
    )?;
    let status = resp.status_code;
    if !(200..300).contains(&status) && status != 207 {
        return Err(format!("PROPFIND failed: HTTP {}", status));
    }
    let text = resp.as_str().map_err(|e| format!("解码失败: {}", e))?;
    let mut out = std::collections::HashMap::new();
    // Parse multi-status XML, extract href + getlastmodified
    for response_block in text.split("<response") {
        if !response_block.contains("<href>") {
            continue;
        }
        let href = extract_between(response_block, "<href>", "</href>").unwrap_or_default();
        let modified_str = extract_between(response_block, "<getlastmodified>", "</getlastmodified>")
            .or_else(|| extract_between(response_block, "<d:getlastmodified>", "</d:getlastmodified>"))
            .unwrap_or_default();
        // Build absolute URL: href is relative to the base
        let full_url = if href.starts_with("http://") || href.starts_with("https://") {
            href.clone()
        } else {
            format!("{}{}", base.trim_end_matches('/'), if href.starts_with('/') { href.to_string() } else { format!("/{}", href) })
        };
        let mtime = parse_http_date(&modified_str);
        out.insert(full_url, mtime);
    }
    Ok(out)
}

fn list_local_profiles(profiles_dir: &PathBuf) -> std::collections::HashMap<String, i64> {
    let mut out = std::collections::HashMap::new();
    if let Ok(entries) = std::fs::read_dir(profiles_dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.ends_with(".txt") {
                    let id = name.trim_end_matches(".txt").to_string();
                    let mtime = file_mtime(&entry.path()).unwrap_or(0);
                    out.insert(id, mtime);
                }
            }
        }
    }
    out
}

fn file_mtime(path: &PathBuf) -> Option<i64> {
    std::fs::metadata(path).ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
}

fn extract_between(text: &str, start: &str, end: &str) -> Option<String> {
    let s = text.find(start)? + start.len();
    let e = text[s..].find(end)? + s;
    Some(decode_xml_entities(&text[s..e]))
}

fn decode_xml_entities(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&amp;", "&")
        .replace("&apos;", "'")
}

fn parse_http_date(s: &str) -> i64 {
    // Parse RFC 1123 / RFC 850 / asctime date formats. Returns unix epoch seconds.
    if s.is_empty() {
        return 0;
    }
    // Try chrono's HTTP date parsing
    if let Ok(dt) = chrono::DateTime::parse_from_rfc2822(s) {
        return dt.timestamp();
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return dt.timestamp();
    }
    0
}

fn base64_encode(input: &str) -> String {
    use std::io::Write;
    let mut buf = Vec::new();
    // Simple base64 encoder to avoid pulling in the `base64` crate
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = input.as_bytes();
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        buf.push(ALPHABET[(b0 >> 2) as usize]);
        buf.push(ALPHABET[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize]);
        if chunk.len() > 1 {
            buf.push(ALPHABET[(((b1 & 0x0F) << 2) | (b2 >> 6)) as usize]);
        } else {
            buf.push(b'=');
        }
        if chunk.len() > 2 {
            buf.push(ALPHABET[(b2 & 0x3F) as usize]);
        } else {
            buf.push(b'=');
        }
    }
    let _ = std::io::sink().write_all(&buf); // satisfy unused write
    String::from_utf8(buf).unwrap_or_default()
}

#[allow(dead_code)]
pub fn now_iso() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    chrono::Local.timestamp_opt(secs, 0).single()
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_default()
}

// ============================ Auto-sync scheduler ============================
// Reactive debounce: every local mutation calls `schedule_sync`, which sets a
// 5-second deadline. A single background task wakes up every 500ms, checks
// the deadline, and runs `sync_now` when it expires. This batches bursts of
// rapid changes (e.g., typing in the editor) into one sync.

pub const DEBOUNCE_SECS: u64 = 5;
const TICK_MS: u64 = 500;

#[derive(Clone)]
pub struct SyncScheduler {
    deadline: Arc<Mutex<Option<Instant>>>,
    app: tauri::AppHandle,
}

/// Global singleton — set once in `setup`, used by mutation commands via
/// `schedule_sync()`. Lives for the lifetime of the app.
static SCHEDULER: OnceCell<SyncScheduler> = OnceCell::new();

pub fn init_scheduler(scheduler: SyncScheduler) {
    let _ = SCHEDULER.set(scheduler);
}

/// Called from mutation commands (create_profile, save_content, etc.)
/// to mark "we want a sync". The background loop fires when the
/// debounce window elapses.
pub fn schedule_sync() {
    if let Some(s) = SCHEDULER.get() {
        s.schedule();
    }
}

/// Run a full sync: load local config, perform WebDAV sync, update
/// status fields in `config.local.json`. Returns the sync result.
/// Used by:
///  - the `sync_now` Tauri command (manual button)
///  - the scheduler loop (debounced push after mutations)
///  - the startup pull task
///  - the periodic background pull task
pub fn sync_now_internal(app: &tauri::AppHandle) -> Result<SyncResult, String> {
    let ctx = crate::storage::Context::Tauri(app);
    let mut local = crate::storage::load_local_config_internal(&ctx)?;
    let url = local.webdav_url.clone().ok_or("WebDAV URL 未配置")?;
    let username = local.webdav_username.clone().ok_or("WebDAV 用户名未配置")?;
    let password = load_credentials(&username)?;
    let cfg = WebDavConfig { url, username };

    let app_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let result = perform_sync(&app_dir, &cfg, &password);

    // Update status fields regardless of success
    match &result {
        Ok(r) => {
            local.webdav_last_sync = Some(now_iso());
            if r.errors.is_empty() {
                local.webdav_last_status = Some("ok".to_string());
            } else {
                local.webdav_last_status = Some(format!("partial: {}", r.errors.len()));
            }
        }
        Err(e) => {
            local.webdav_last_sync = Some(now_iso());
            local.webdav_last_status = Some(format!("error: {}", e));
        }
    }
    let _ = crate::storage::save_local_config_internal(&ctx, &local);

    result
}

impl SyncScheduler {
    pub fn new(app: tauri::AppHandle) -> Self {
        Self {
            deadline: Arc::new(Mutex::new(None)),
            app,
        }
    }

    /// Mark "we want to sync". The actual sync runs DEBOUNCE_SECS later.
    pub fn schedule(&self) {
        let new_deadline = Instant::now() + Duration::from_secs(DEBOUNCE_SECS);
        if let Ok(mut d) = self.deadline.try_lock() {
            *d = Some(new_deadline);
        }
    }

    /// Force an immediate sync (skip the debounce). Used at app startup
    /// and by the periodic background pull.
    pub async fn run_immediate(&self) -> Result<SyncResult, String> {
        sync_now_internal(&self.app)
    }

    /// Long-running task: every TICK_MS, check the deadline. If it's
    /// elapsed, run a sync.
    pub async fn run_loop(self) {
        let mut tick = interval(Duration::from_millis(TICK_MS));
        loop {
            tick.tick().await;
            let should_fire = {
                let mut d = match self.deadline.try_lock() {
                    Ok(g) => g,
                    Err(_) => continue, // Skip this tick if mutex is busy
                };
                if let Some(deadline) = *d {
                    if Instant::now() >= deadline {
                        *d = None;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            };
            if should_fire {
                let result = sync_now_internal(&self.app);
                if let Err(e) = &result {
                    eprintln!("Scheduled sync failed: {}", e);
                }
            }
        }
    }
}

/// Warn the user if their local changes are stale (older than ~30 days)
/// relative to the remote. With last-write-wins, stale local changes that
/// silently push would clobber fresher remote work.
const STALE_WARNING_DAYS: i64 = 30;

pub fn check_staleness(local_mtime: i64, remote_mtime: i64) -> Option<String> {
    let diff = local_mtime - remote_mtime;
    let days = diff / 86400;
    if days > STALE_WARNING_DAYS {
        Some(format!(
            "本地修改时间比远端新 {} 天,推送可能覆盖他人近期改动",
            days
        ))
    } else {
        None
    }
}
