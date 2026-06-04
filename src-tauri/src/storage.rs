use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProfileMetadata {
    pub id: String,
    pub name: String,
    pub active: bool,
    /// Remote URL for downloading hosts (if applicable)
    pub url: Option<String>,
    /// Last successful update timestamp (ISO 8601)
    pub last_update: Option<String>,
    /// Auto-update interval in seconds (0 or None means manual)
    pub update_interval: Option<u64>,
    /// SHA-256 of the last content successfully uploaded to the WebDAV
    /// remote. Used to skip redundant PUTs when the local file hasn't
    /// actually changed (mtime may differ between local FS precision and
    /// HTTP Last-Modified second precision). `#[serde(default)]` keeps
    /// backward compat with existing `config.sync.json` files.
    #[serde(default)]
    pub last_hash: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AppConfig {
    pub multi_select: bool,
    pub theme: Option<String>,
    pub window_mode: Option<String>,
    pub window_width: Option<f64>,
    pub window_height: Option<f64>,
    pub sidebar_width: Option<f64>,
    pub profiles: Vec<ProfileMetadata>,
    pub active_profile_ids: Vec<String>,
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default = "default_close_behavior")]
    pub close_behavior: String,
    #[serde(default)]
    pub remember_close_choice: bool,
    // WebDAV sync fields (kept here so AppConfig is still a complete view)
    #[serde(default)]
    pub webdav_url: Option<String>,
    #[serde(default)]
    pub webdav_username: Option<String>,
    #[serde(default)]
    pub webdav_last_sync: Option<String>,
    /// Format version of the on-disk files. 1 = old single config.json,
    /// 2 = split (config.local.json + config.sync.json). Defaults to 2
    /// when absent.
    #[serde(default = "default_config_version")]
    pub config_version: u32,
}

fn default_close_behavior() -> String {
    "exit".to_string()
}

fn default_config_version() -> u32 {
    2
}

/// Per-device, **NOT** synced via WebDAV. Stored in `config.local.json`.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct LocalConfig {
    pub theme: Option<String>,
    pub window_mode: Option<String>,
    pub window_width: Option<f64>,
    pub window_height: Option<f64>,
    pub sidebar_width: Option<f64>,
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default = "default_close_behavior")]
    pub close_behavior: String,
    #[serde(default)]
    pub remember_close_choice: bool,
    #[serde(default)]
    pub webdav_url: Option<String>,
    #[serde(default)]
    pub webdav_username: Option<String>,
    #[serde(default)]
    pub webdav_last_sync: Option<String>,
    /// Last sync operation's status: "ok" / "error: ..." / "never"
    #[serde(default)]
    pub webdav_last_status: Option<String>,
}

impl From<&AppConfig> for LocalConfig {
    fn from(c: &AppConfig) -> Self {
        Self {
            theme: c.theme.clone(),
            window_mode: c.window_mode.clone(),
            window_width: c.window_width,
            window_height: c.window_height,
            sidebar_width: c.sidebar_width,
            auto_start: c.auto_start,
            close_behavior: c.close_behavior.clone(),
            remember_close_choice: c.remember_close_choice,
            webdav_url: c.webdav_url.clone(),
            webdav_username: c.webdav_username.clone(),
            webdav_last_sync: c.webdav_last_sync.clone(),
            webdav_last_status: None,
        }
    }
}

/// Cross-device, **synced** via WebDAV. Stored in `config.sync.json`.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SyncConfig {
    pub multi_select: bool,
    pub profiles: Vec<ProfileMetadata>,
    pub active_profile_ids: Vec<String>,
}

impl From<&AppConfig> for SyncConfig {
    fn from(c: &AppConfig) -> Self {
        Self {
            multi_select: c.multi_select,
            profiles: c.profiles.clone(),
            active_profile_ids: c.active_profile_ids.clone(),
        }
    }
}

impl From<&LocalConfig> for AppConfig {
    fn from(l: &LocalConfig) -> Self {
        Self {
            multi_select: false,
            theme: l.theme.clone(),
            window_mode: l.window_mode.clone(),
            window_width: l.window_width,
            window_height: l.window_height,
            sidebar_width: l.sidebar_width,
            profiles: Vec::new(),
            active_profile_ids: Vec::new(),
            auto_start: l.auto_start,
            close_behavior: l.close_behavior.clone(),
            remember_close_choice: l.remember_close_choice,
            webdav_url: l.webdav_url.clone(),
            webdav_username: l.webdav_username.clone(),
            webdav_last_sync: l.webdav_last_sync.clone(),
            config_version: 2,
        }
    }
}

impl From<&SyncConfig> for AppConfig {
    fn from(s: &SyncConfig) -> Self {
        Self {
            multi_select: s.multi_select,
            theme: None,
            window_mode: None,
            window_width: None,
            window_height: None,
            sidebar_width: None,
            profiles: s.profiles.clone(),
            active_profile_ids: s.active_profile_ids.clone(),
            auto_start: false,
            close_behavior: "exit".to_string(),
            remember_close_choice: false,
            webdav_url: None,
            webdav_username: None,
            webdav_last_sync: None,
            config_version: 2,
        }
    }
}

impl AppConfig {
    /// Merge LocalConfig + SyncConfig into a full AppConfig (used for UI consumption).
    pub fn merge(local: &LocalConfig, sync: &SyncConfig) -> Self {
        let mut c = AppConfig::from(local);
        c.multi_select = sync.multi_select;
        c.profiles = sync.profiles.clone();
        c.active_profile_ids = sync.active_profile_ids.clone();
        c
    }

    /// Split AppConfig into its local and sync parts (used at save time).
    pub fn split(&self) -> (LocalConfig, SyncConfig) {
        (LocalConfig::from(self), SyncConfig::from(self))
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProfileData {
    pub id: String,
    pub name: String,
    pub content: String,
    pub active: bool,
}

pub enum Context<'a> {
    Tauri(&'a AppHandle),
    Headless,
}

impl<'a> Context<'a> {
    pub fn get_app_dir(&self) -> Result<PathBuf, String> {
        match self {
            Context::Tauri(app) => app.path().app_data_dir().map_err(|e| e.to_string()),
            Context::Headless => {
                // Hardcoded fallback for headless CLI to match Tauri's app_data_dir for "com.hostly.app"
                #[cfg(target_os = "windows")]
                {
                    let base = std::env::var("APPDATA").map(PathBuf::from).map_err(|_| "APPDATA env var not found")?;
                    Ok(base.join("com.hostly.switcher"))
                }
                #[cfg(target_os = "macos")]
                {
                    let home = std::env::var("HOME").map(PathBuf::from).map_err(|_| "HOME env var not found")?;
                    Ok(home.join("Library/Application Support/com.hostly.switcher"))
                }
                #[cfg(target_os = "linux")]
                {
                    if let Ok(data_home) = std::env::var("XDG_DATA_HOME") {
                        Ok(PathBuf::from(data_home).join("com.hostly.switcher"))
                    } else {
                        let home = std::env::var("HOME").map(PathBuf::from).map_err(|_| "HOME env var not found")?;
                        Ok(home.join(".local/share/com.hostly.switcher"))
                    }
                }
            }
        }
    }
}

fn get_profiles_dir(ctx: &Context) -> Result<PathBuf, String> {
    let dir = ctx.get_app_dir()?.join("profiles");
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    Ok(dir)
}

fn get_config_path(ctx: &Context) -> Result<PathBuf, String> {
    Ok(ctx.get_app_dir()?.join("config.json"))
}

fn get_local_config_path(ctx: &Context) -> Result<PathBuf, String> {
    Ok(ctx.get_app_dir()?.join("config.local.json"))
}

fn get_sync_config_path(ctx: &Context) -> Result<PathBuf, String> {
    Ok(ctx.get_app_dir()?.join("config.sync.json"))
}

fn get_common_path(ctx: &Context) -> Result<PathBuf, String> {
    Ok(ctx.get_app_dir()?.join("common.txt"))
}

#[tauri::command]
pub fn load_config(app: AppHandle) -> Result<AppConfig, String> {
    load_config_internal(&Context::Tauri(&app))
}

pub fn load_config_internal(ctx: &Context) -> Result<AppConfig, String> {
    let local_path = get_local_config_path(ctx)?;
    let sync_path = get_sync_config_path(ctx)?;
    let legacy_path = get_config_path(ctx)?;

    // New format: both files exist — load and merge
    if local_path.exists() && sync_path.exists() {
        let local_content = fs::read_to_string(&local_path).map_err(|e| e.to_string())?;
        let sync_content = fs::read_to_string(&sync_path).map_err(|e| e.to_string())?;
        let local: LocalConfig = serde_json::from_str(&local_content).map_err(|e| e.to_string())?;
        let sync: SyncConfig = serde_json::from_str(&sync_content).map_err(|e| e.to_string())?;
        return Ok(AppConfig::merge(&local, &sync));
    }

    // Migration: old single config.json exists — load, split, write new files
    if legacy_path.exists() {
        let content = fs::read_to_string(&legacy_path).map_err(|e| e.to_string())?;
        let old: AppConfig = serde_json::from_str(&content).map_err(|e| e.to_string())?;
        let (local, sync) = old.split();
        save_local_config_internal(ctx, &local)?;
        save_sync_config_internal(ctx, &sync)?;
        // Keep legacy file as backup for one release, but mark migrated.
        // Don't delete it — safer in case of corruption.
        return Ok(old);
    }

    // First run: create defaults in new format
    create_default_config(ctx)
}

pub fn save_config_internal(ctx: &Context, config: &AppConfig) -> Result<(), String> {
    let (local, sync) = config.split();
    save_local_config_internal(ctx, &local)?;
    save_sync_config_internal(ctx, &sync)?;
    Ok(())
}

pub fn save_local_config_internal(ctx: &Context, local: &LocalConfig) -> Result<(), String> {
    let path = get_local_config_path(ctx)?;
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    let content = serde_json::to_string_pretty(local).map_err(|e| e.to_string())?;
    fs::write(path, content).map_err(|e| e.to_string())
}

pub fn save_sync_config_internal(ctx: &Context, sync: &SyncConfig) -> Result<(), String> {
    let path = get_sync_config_path(ctx)?;
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    let content = serde_json::to_string_pretty(sync).map_err(|e| e.to_string())?;
    fs::write(path, content).map_err(|e| e.to_string())
}

pub fn load_local_config_internal(ctx: &Context) -> Result<LocalConfig, String> {
    let path = get_local_config_path(ctx)?;
    if !path.exists() {
        return Ok(LocalConfig::default());
    }
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
}

pub fn load_sync_config_internal(ctx: &Context) -> Result<SyncConfig, String> {
    let path = get_sync_config_path(ctx)?;
    if !path.exists() {
        return Ok(SyncConfig::default());
    }
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
}

fn create_default_config(ctx: &Context) -> Result<AppConfig, String> {
    let mut sync = SyncConfig::default();
    sync.multi_select = false;
    let mut local = LocalConfig::default();
    local.auto_start = false;
    local.close_behavior = "exit".to_string();
    local.remember_close_choice = false;

    let defaults = vec!["Dev", "Test", "Prod"];

    // 1. Auto-backup System Hosts
    let sys_id = Uuid::new_v4().to_string();
    let sys_hosts_content = crate::hosts::get_system_hosts();
    let sys_content = sys_hosts_content.unwrap_or_else(|_| "# Backup failed".to_string());

    save_profile_file_internal(ctx, &sys_id, &sys_content)?;
    sync.profiles.push(ProfileMetadata {
        id: sys_id,
        name: "系统hosts备份".to_string(),
        active: false,
        url: None,
        last_update: None,
        update_interval: None,
        last_hash: None,
    });

    // 2. Default Envs
    for name in defaults {
        let id = Uuid::new_v4().to_string();
        save_profile_file_internal(ctx, &id, "# New Environment\n")?;
        sync.profiles.push(ProfileMetadata {
            id,
            name: name.to_string(),
            active: false,
            url: None,
            last_update: None,
            update_interval: None,
            last_hash: None,
        });
    }

    // Save new format
    save_local_config_internal(ctx, &local)?;
    save_sync_config_internal(ctx, &sync)?;

    Ok(AppConfig::merge(&local, &sync))
}

pub fn save_profile_file_internal(ctx: &Context, id: &str, content: &str) -> Result<(), String> {
    let dir = get_profiles_dir(ctx)?;
    let path = dir.join(format!("{}.txt", id));
    fs::write(path, content).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn load_common_config(app: AppHandle) -> Result<String, String> {
    load_common_config_internal(&Context::Tauri(&app))
}

pub fn load_common_config_internal(ctx: &Context) -> Result<String, String> {
    let path = get_common_path(ctx)?;
    if !path.exists() {
        return Ok(String::new());
    }
    fs::read_to_string(path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_common_config(app: AppHandle, content: String) -> Result<(), String> {
    save_common_config_internal(&Context::Tauri(&app), content)?;
    apply_config(app)
}

pub fn save_common_config_internal(ctx: &Context, content: String) -> Result<(), String> {
    let path = get_common_path(ctx)?;
    fs::write(path, content).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_theme(app: AppHandle, theme: String) -> Result<(), String> {
    set_theme_internal(&Context::Tauri(&app), theme)
}

pub fn set_theme_internal(ctx: &Context, theme: String) -> Result<(), String> {
    let mut config = load_config_internal(ctx)?;
    config.theme = Some(theme);
    save_config_internal(ctx, &config)
}

#[tauri::command]
pub fn save_window_config(app: AppHandle, mode: String, width: f64, height: f64) -> Result<(), String> {
    save_window_config_internal(&Context::Tauri(&app), mode, width, height)
}

pub fn save_window_config_internal(ctx: &Context, mode: String, width: f64, height: f64) -> Result<(), String> {
    let mut config = load_config_internal(ctx)?;
    config.window_mode = Some(mode);
    config.window_width = Some(width);
    config.window_height = Some(height);
    save_config_internal(ctx, &config)
}

#[tauri::command]
pub fn save_sidebar_config(app: AppHandle, width: f64) -> Result<(), String> {
    save_sidebar_config_internal(&Context::Tauri(&app), width)
}

pub fn save_sidebar_config_internal(ctx: &Context, width: f64) -> Result<(), String> {
    let mut config = load_config_internal(ctx)?;
    config.sidebar_width = Some(width);
    save_config_internal(ctx, &config)
}

#[tauri::command]
pub fn list_profiles(app: AppHandle) -> Result<Vec<ProfileData>, String> {
    list_profiles_internal(&Context::Tauri(&app))
}

pub fn list_profiles_internal(ctx: &Context) -> Result<Vec<ProfileData>, String> {
    let config = load_config_internal(ctx)?;
    let dir = get_profiles_dir(ctx)?;
    
    let mut profiles = Vec::new();
    
    for meta in config.profiles {
        let path = dir.join(format!("{}.txt", meta.id));
        let content = if path.exists() {
             fs::read_to_string(&path).unwrap_or_default()
        } else {
             String::new()
        };
        
        profiles.push(ProfileData {
            id: meta.id,
            name: meta.name,
            content,
            active: meta.active,
        });
    }
    
    Ok(profiles)
}

#[tauri::command]
pub fn create_profile(
    app: AppHandle,
    name: String,
    content: Option<String>,
    url: Option<String>,
    update_interval: Option<u64>
) -> Result<String, String> {
    let id = create_profile_internal(&Context::Tauri(&app), name, content, url, update_interval)?;
    crate::webdav::schedule_sync();
    Ok(id)
}

pub fn create_profile_internal(
    ctx: &Context,
    name: String,
    content: Option<String>,
    url: Option<String>,
    update_interval: Option<u64>
) -> Result<String, String> {
    let mut config = load_config_internal(ctx)?;
    
    // Check for duplicate name
    if config.profiles.iter().any(|p| p.name == name) {
        return Err("环境名称已存在 / Profile name already exists".to_string());
    }

    let id = Uuid::new_v4().to_string();
    let initial_content = content.unwrap_or_default();
    save_profile_file_internal(ctx, &id, &initial_content)?;
    
    config.profiles.push(ProfileMetadata {
        id: id.clone(),
        name,
        active: false,
        url,
        last_update: None,
        update_interval,
        last_hash: None,
    });
    
    save_config_internal(ctx, &config)?;
    Ok(id)
}

#[tauri::command]
pub fn save_profile_content(app: AppHandle, id: String, content: String) -> Result<(), String> {
    let ctx = Context::Tauri(&app);
    save_profile_content_internal(&ctx, &id, &content)?;

    // If this profile is active, re-apply config to system hosts
    let config = load_config_internal(&ctx)?;
    if config.profiles.iter().any(|p| p.id == id && p.active) {
        apply_config(app)?;
    }
    crate::webdav::schedule_sync();
    Ok(())
}

pub fn save_profile_content_internal(ctx: &Context, id: &str, content: &str) -> Result<(), String> {
    save_profile_file_internal(ctx, id, content)
}

#[tauri::command]
pub fn delete_profile(app: AppHandle, id: String) -> Result<(), String> {
    delete_profile_internal(&Context::Tauri(&app), &id)?;
    crate::webdav::schedule_sync();
    Ok(())
}

pub fn delete_profile_internal(ctx: &Context, id: &str) -> Result<(), String> {
    let mut config = load_config_internal(ctx)?;
    
    // Remove from config
    if let Some(idx) = config.profiles.iter().position(|p| p.id == id) {
        config.profiles.remove(idx);
        save_config_internal(ctx, &config)?;
    }
    
    // Delete file
    let dir = get_profiles_dir(ctx)?;
    let path = dir.join(format!("{}.txt", id));
    if path.exists() {
        let _ = fs::remove_file(path);
    }
    
    Ok(())
}

#[tauri::command]
pub fn rename_profile(app: AppHandle, id: String, new_name: String) -> Result<(), String> {
    rename_profile_internal(&Context::Tauri(&app), &id, new_name)?;
    crate::webdav::schedule_sync();
    Ok(())
}

pub fn rename_profile_internal(ctx: &Context, id: &str, new_name: String) -> Result<(), String> {
    let mut config = load_config_internal(ctx)?;
    
    // Check for duplicate name (excluding itself)
    if config.profiles.iter().any(|p| p.name == new_name && p.id != id) {
        return Err("环境名称已存在 / Profile name already exists".to_string());
    }

    if let Some(idx) = config.profiles.iter().position(|p| p.id == id) {
        config.profiles[idx].name = new_name;
        save_config_internal(ctx, &config)?;
    }
    Ok(())
}

#[tauri::command]
pub fn toggle_profile_active(app: AppHandle, id: String) -> Result<(), String> {
    toggle_profile_active_internal(&Context::Tauri(&app), &id)?;
    apply_config(app.clone())?;
    // 同步托盘菜单 ✓ 标记(读 active_profile_ids,toggle 后已 sync 过)
    crate::rebuild_tray_menu(&app);
    crate::webdav::schedule_sync();
    Ok(())
}

pub fn toggle_profile_active_internal(ctx: &Context, id: &str) -> Result<(), String> {
    let mut config = load_config_internal(ctx)?;
    
    if config.multi_select {
        // Toggle specific
        if let Some(p) = config.profiles.iter_mut().find(|p| p.id == id) {
            p.active = !p.active;
        }
    } else {
        // Single select logic
        // If clicking active, toggle off? Or do nothing? Usually toggle off or keep on.
        // Let's say toggle off if already on.
        let was_active = config.profiles.iter().find(|p| p.id == id).map(|p| p.active).unwrap_or(false);
        
        // Turn all off
        for p in &mut config.profiles {
            p.active = false;
        }
        
        // If it wasn't active, turn it on
        if !was_active {
            if let Some(p) = config.profiles.iter_mut().find(|p| p.id == id) {
                p.active = true;
            }
        }
    }

    // 同步 active_profile_ids (托盘 ✓ 标记读这个),否则托盘不更新
    sync_active_profile_ids(&mut config);

    save_config_internal(ctx, &config)
}

/// Sync active_profile_ids (Vec<String>) with profiles[i].active (per-profile bool).
/// MUST be called after any code path that flips profiles[i].active, otherwise the
/// tray menu's ✓ markers (which read active_profile_ids) will go stale.
fn sync_active_profile_ids(config: &mut AppConfig) {
    config.active_profile_ids = config
        .profiles
        .iter()
        .filter(|p| p.active)
        .map(|p| p.id.clone())
        .collect();
}

#[tauri::command]
pub fn set_multi_select(app: AppHandle, enable: bool) -> Result<(), String> {
    set_multi_select_internal(&Context::Tauri(&app), enable)?;
    apply_config(app)?;
    crate::webdav::schedule_sync();
    Ok(())
}

pub fn set_multi_select_internal(ctx: &Context, enable: bool) -> Result<(), String> {
    let mut config = load_config_internal(ctx)?;
    config.multi_select = enable;
    
    // If disabling multi-select, and multiple are active, keep only first
    if !enable {
        let mut found = false;
        for p in &mut config.profiles {
            if p.active {
                if found {
                    p.active = false;
                } else {
                    found = true;
                }
            }
        }
    }

    // 同步 active_profile_ids (托盘 ✓ 标记读这个)
    sync_active_profile_ids(&mut config);

    save_config_internal(ctx, &config)
}

#[tauri::command]
pub fn apply_config(app: AppHandle) -> Result<(), String> {
    apply_config_internal(&Context::Tauri(&app))
}

pub fn apply_config_internal(ctx: &Context) -> Result<(), String> {
    let config = load_config_internal(ctx)?;
    let common_config = load_common_config_internal(ctx).unwrap_or_default();
    
    let profiles_dir = get_profiles_dir(ctx)?;
    let mut merged_content = String::from("# Generated by Hostly\n\n");
    merged_content.push_str("### Common Config ###\n");
    merged_content.push_str(&common_config);
    merged_content.push_str("\n\n");

    let read_profile = |id: &str| -> String {
        let path = profiles_dir.join(format!("{}.txt", id));
        if path.exists() {
             fs::read_to_string(path).unwrap_or_default()
        } else {
             String::new()
        }
    };

    for profile in config.profiles {
        if profile.active {
            merged_content.push_str(&format!("### Profile: {} ###\n", profile.name));
            merged_content.push_str(&read_profile(&profile.id));
            merged_content.push_str("\n\n");
        }
    }

    crate::hosts::save_system_hosts(merged_content)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FullBackup {
    version: i32,
    timestamp: String,
    config: AppConfig,
    // Support both new (Vec) and old (HashMap) formats for compatibility
    profiles: Option<Vec<ProfileData>>,
    profiles_content: Option<std::collections::HashMap<String, String>>,
}

#[tauri::command]
pub fn import_data(app: AppHandle, json_content: String) -> Result<(), String> {
    import_data_internal(&Context::Tauri(&app), json_content)?;
    apply_config(app)
}

pub fn import_data_internal(ctx: &Context, json_content: String) -> Result<(), String> {
    let backup: FullBackup = serde_json::from_str(&json_content).map_err(|e| e.to_string())?;
    
    // Reset config
    save_config_internal(ctx, &backup.config)?;
    
    // Save each profile (New Version: Vec<ProfileData>)
    if let Some(profiles) = backup.profiles {
        for profile in profiles {
            save_profile_file_internal(ctx, &profile.id, &profile.content)?;
        }
    } 
    // Save each profile (Old Version: HashMap<id, content>)
    else if let Some(profiles_content) = backup.profiles_content {
        for (id, content) in profiles_content {
            save_profile_file_internal(ctx, &id, &content)?;
        }
    }
    
    Ok(())
}

#[tauri::command]
pub fn export_data(app: AppHandle) -> Result<String, String> {
    export_data_internal(&Context::Tauri(&app))
}

pub fn export_data_internal(ctx: &Context) -> Result<String, String> {
    let config = load_config_internal(ctx)?;
    let profiles = list_profiles_internal(ctx)?;
    
    let backup = FullBackup {
        version: 2,
        timestamp: chrono::Local::now().to_rfc3339(),
        config,
        profiles: Some(profiles),
        profiles_content: None,
    };
    
    serde_json::to_string_pretty(&backup).map_err(|e| e.to_string())
}

// Helpers for simple file io not needed as much now, but kept for single export if needed
#[tauri::command]
pub fn import_file(path: String) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn export_file(path: String, content: String) -> Result<(), String> {
    fs::write(path, content).map_err(|e| e.to_string())
}

// ================= CLI Helpers =================
// These functions are pub but not commands, used by cli.rs
#[tauri::command]
pub fn find_profile_id_by_name(app: AppHandle, name: String) -> Result<Option<String>, String> {
    find_profile_id_by_name_internal(&Context::Tauri(&app), &name)
}

pub fn find_profile_id_by_name_internal(ctx: &Context, name: &str) -> Result<Option<String>, String> {
    let config = load_config_internal(ctx)?;
    Ok(config.profiles.iter().find(|p| p.name == name).map(|p| p.id.clone()))
}

#[tauri::command]
pub fn upsert_profile(app: AppHandle, name: String, content: String) -> Result<String, String> {
    upsert_profile_internal(&Context::Tauri(&app), name, content)
}

pub fn upsert_profile_internal(ctx: &Context, name: String, content: String) -> Result<String, String> {
    if let Some(id) = find_profile_id_by_name_internal(ctx, &name)? {
        save_profile_file_internal(ctx, &id, &content)?;
        Ok(id)
    } else {

        create_profile_internal(ctx, name, Some(content), None, None)
    }
}

#[tauri::command]
pub fn import_switchhosts(app: AppHandle, json_content: String) -> Result<usize, String> {
    let ctx = Context::Tauri(&app);
    let count = import_switchhosts_internal(&ctx, json_content)?;
    apply_config(app)?;
    Ok(count)
}

pub fn import_switchhosts_internal(ctx: &Context, json_content: String) -> Result<usize, String> {
    let raw: serde_json::Value = serde_json::from_str(&json_content).map_err(|e| format!("Invalid JSON: {}", e))?;
    
    // SwitchHosts v4+ format: data.list.tree (structure) + data.collection.hosts.data (content)
    if let Some(data) = raw.get("data") {
        let mut content_map = std::collections::HashMap::new();
        
        // Build ID -> Content map
        if let Some(hosts_data) = data.get("collection")
            .and_then(|c| c.get("hosts"))
            .and_then(|h| h.get("data"))
            .and_then(|d| d.as_array()) 
        {
            for h in hosts_data {
                if let (Some(id), Some(content)) = (h.get("id").and_then(|v| v.as_str()), h.get("content").and_then(|v| v.as_str())) {
                    content_map.insert(id, content);
                }
            }
        }

        // Traverse tree
        if let Some(tree) = data.get("list").and_then(|l| l.get("tree")).and_then(|t| t.as_array()) {
            let mut count = 0;
            parse_switchhosts_v4_tree_internal(ctx, tree, &content_map, &mut count)?;
            return Ok(count);
        }
    }

    // Fallback to simpler format (v1-v3 or simpler exports)
    let list = if let Some(l) = raw.get("list") {
        l.as_array().ok_or("Invalid SwitchHosts format: 'list' is not an array")?
    } else if raw.is_array() {
        raw.as_array().unwrap()
    } else {
        return Err("Invalid SwitchHosts format: Expected SH v4 structure or a simple array".to_string());
    };

    let mut count = 0;
    parse_switchhosts_items_internal(ctx, list, &mut count)?;

    Ok(count)
}

fn parse_switchhosts_v4_tree_internal(
    ctx: &Context, 
    items: &Vec<serde_json::Value>, 
    content_map: &std::collections::HashMap<&str, &str>, 
    count: &mut usize
) -> Result<(), String> {
    for item in items {
        let title = item.get("title").and_then(|v| v.as_str()).unwrap_or("Unknown");
        let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("local");
        let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("");

        if item_type == "folder" {
            if let Some(children) = item.get("children").and_then(|c| c.as_array()) {
                parse_switchhosts_v4_tree_internal(ctx, children, content_map, count)?;
            }
        } else {
            // Find content in map or item itself
            let content = content_map.get(id).map(|c| *c).or_else(|| item.get("content").and_then(|v| v.as_str())).unwrap_or("");
            upsert_profile_internal(ctx, title.to_string(), content.to_string())?;
            *count += 1;
        }
    }
    Ok(())
}

fn parse_switchhosts_items_internal(ctx: &Context, items: &Vec<serde_json::Value>, count: &mut usize) -> Result<(), String> {
    for item in items {
        let title = item.get("title").and_then(|v| v.as_str()).unwrap_or("Unknown");
        let folder = item.get("folder").and_then(|v| v.as_bool())
            .or_else(|| item.get("type").and_then(|v| Some(v.as_str() == Some("folder"))))
            .unwrap_or(false);
        
        if folder {
            if let Some(children) = item.get("children").and_then(|c| c.as_array()) {
                parse_switchhosts_items_internal(ctx, children, count)?;
            }
        } else {
            let content = item.get("content").and_then(|v| v.as_str()).unwrap_or("");
            upsert_profile_internal(ctx, title.to_string(), content.to_string())?;
            *count += 1;
        }
    }

    Ok(())
}

pub fn check_auto_updates(app: &AppHandle) {
    let ctx = Context::Tauri(app);
    // Silent check, allow errors to just print to stderr
    if let Ok(config) = load_config_internal(&ctx) {
        let now = chrono::Local::now();
        let mut needs_save = false;
        
        // Collect IDs to update to avoid borrow checker issues with iterating & mutating config
        let mut updates_needed = Vec::new();

        for p in &config.profiles {
            if let (Some(_url), Some(interval), Some(last_update_str)) = (&p.url, p.update_interval, &p.last_update) {
                if interval > 0 {
                    if let Ok(last_update) = chrono::DateTime::parse_from_rfc3339(last_update_str) {
                        let diff = now.signed_duration_since(last_update);
                        if diff.num_seconds() >= interval as i64 {
                            updates_needed.push(p.id.clone());
                        }
                    }
                }
            } else if let (Some(_url), Some(interval), None) = (&p.url, p.update_interval, &p.last_update) {
                // Never updated, but has interval -> update now
                 if interval > 0 {
                    updates_needed.push(p.id.clone());
                 }
            }
        }
        
        for id in updates_needed {
            println!("Auto-updating profile {}...", id);
            if let Err(e) = trigger_profile_update_internal(&ctx, &id) {
                eprintln!("Failed to auto-update {}: {}", id, e);
            }
            // re-application is handled inside trigger_profile_update_internal? 
            // implementation_plan said Trigger triggers re-apply. 
            // Actually `trigger_profile_update` command does, but `internal` does NOT re-apply.
            // We should reload config to check if active and apply if needed?
            // checking internal implementation...
            // `trigger_profile_update_internal` saves file and updates timestamp in config.
            // It does NOT call apply_config.
            // So we need to do it here if any update happened.
            needs_save = true;
        }

        if needs_save {
             // Re-apply config if any active profile was updated
             // Optimization: check if any updated profile was active
             // For now, just apply to be safe
             let _ = apply_config_internal(&ctx);
        }
    }
}

#[tauri::command]
pub fn update_remote_config(
    app: AppHandle,
    id: String,
    url: Option<String>,
    update_interval: Option<u64>
) -> Result<(), String> {
    let ctx = Context::Tauri(&app);
    let mut config = load_config_internal(&ctx)?;

    if let Some(p) = config.profiles.iter_mut().find(|p| p.id == id) {
        p.url = url;
        p.update_interval = update_interval;
    } else {
        return Err("Profile not found".to_string());
    }

    save_config_internal(&ctx, &config)?;
    crate::webdav::schedule_sync();
    Ok(())
}

#[tauri::command]
pub fn trigger_profile_update(app: AppHandle, id: String) -> Result<(), String> {
    let ctx = Context::Tauri(&app);
    trigger_profile_update_internal(&ctx, &id)?;
    // If active, re-apply
    let config = load_config_internal(&ctx)?;
    if config.profiles.iter().any(|p| p.id == id && p.active) {
        apply_config(app)?;
    }
    Ok(())
}

pub fn trigger_profile_update_internal(ctx: &Context, id: &str) -> Result<(), String> {
    let mut config = load_config_internal(ctx)?;
    
    let (url, name) = if let Some(p) = config.profiles.iter().find(|p| p.id == id) {
        (p.url.clone(), p.name.clone())
    } else {
        return Err("Profile not found".to_string());
    };

    let url = url.ok_or("Profile is not a remote profile (no URL)")?;
    
    // Download
    println!("Downloading profile '{}' from '{}'...", name, url);
    let content = download_text(&url)?;

    // Save Content
    save_profile_file_internal(ctx, id, &content)?;

    // Update Timestamp
    if let Some(p) = config.profiles.iter_mut().find(|p| p.id == id) {
        p.last_update = Some(chrono::Local::now().to_rfc3339());
    }
    save_config_internal(ctx, &config)?;
    
    Ok(())
}

fn download_text(urls_str: &str) -> Result<String, String> {
    let mut combined_content = String::new();
    let urls: Vec<&str> = urls_str.lines().map(|s| s.trim()).filter(|s| !s.is_empty()).collect();

    if urls.is_empty() {
        return Err("No valid URLs provided".to_string());
    }

    for url in urls {
        let content = download_single_url(url)?;
        if !combined_content.is_empty() {
            combined_content.push_str("\n\n");
        }
        combined_content.push_str(&format!("# Source: {}\n", url));
        combined_content.push_str(&content);
    }

    Ok(combined_content)
}

fn download_single_url(url: &str) -> Result<String, String> {
    let response = minreq::get(url)
        .with_timeout(10)
        .send()
        .map_err(|e| format!("Network error downloading {}: {}", url, e))?;
        
    if response.status_code >= 200 && response.status_code < 300 {
        response.as_str().map(|s| s.to_string()).map_err(|e| format!("Invalid text encoding from {}: {}", url, e))
    } else {
        Err(format!("HTTP Error {} from {}", response.status_code, url))
    }
}

#[tauri::command]
pub fn set_auto_start(app: AppHandle, enable: bool) -> Result<(), String> {
    let ctx = Context::Tauri(&app);
    crate::autostart::set_auto_start(&app, enable)?;
    let mut config = load_config_internal(&ctx)?;
    config.auto_start = enable;
    save_config_internal(&ctx, &config)
}

#[tauri::command]
pub fn get_auto_start(app: AppHandle) -> Result<bool, String> {
    Ok(crate::autostart::is_auto_start_enabled(&app))
}

#[tauri::command]
pub fn save_close_behavior(app: AppHandle, behavior: String) -> Result<(), String> {
    let ctx = Context::Tauri(&app);
    let mut config = load_config_internal(&ctx)?;
    config.close_behavior = behavior;
    save_config_internal(&ctx, &config)
}

#[tauri::command]
pub fn get_close_behavior(app: AppHandle) -> Result<String, String> {
    let ctx = Context::Tauri(&app);
    let config = load_config_internal(&ctx)?;
    Ok(config.close_behavior)
}

#[tauri::command]
pub fn save_remember_close_choice(app: AppHandle, remember: bool) -> Result<(), String> {
    let ctx = Context::Tauri(&app);
    let mut config = load_config_internal(&ctx)?;
    config.remember_close_choice = remember;
    save_config_internal(&ctx, &config)
}

#[tauri::command]
pub fn get_remember_close_choice(app: AppHandle) -> Result<bool, String> {
    let ctx = Context::Tauri(&app);
    let config = load_config_internal(&ctx)?;
    Ok(config.remember_close_choice)
}

