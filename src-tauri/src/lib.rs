mod hosts;
pub mod storage;
pub mod cli;
pub mod elevation;
pub mod autostart;
pub mod webdav;

#[cfg(target_os = "windows")]
use window_vibrancy::apply_mica;
use tauri::{
    Manager,
    Emitter,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    menu::{Menu, MenuItem, PredefinedMenuItem, SubmenuBuilder},
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostarted"]),
        ))
        .setup(|app| {
            if cli::run_cli(Some(&app.handle())) {
                std::process::exit(0);
            }

            let window = app.get_webview_window("main").unwrap();

            // tauri 2 不替换 tauri.conf.json title 里的 {{version}} 模板;
            // 运行时把版本号拼进标题
            let title = format!(
                "Hostlyy v{}",
                app.package_info().version
            );
            let _ = window.set_title(&title);

            #[cfg(target_os = "windows")]
            {
                let _ = apply_mica(&window, Some(true));
            }
            
            let ctx = storage::Context::Tauri(&app.handle());
            if let Ok(config) = storage::load_config_internal(&ctx) {
                if let (Some(w), Some(h)) = (config.window_width, config.window_height) {
                     let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize { width: w, height: h }));
                }
            }

            let show_item = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            let _tray_handle = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .tooltip("Hostlyy")
                .on_menu_event(move |app, event| {
                    match event.id.as_ref() {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        id if id.starts_with("profile:") => {
                            // 托盘子菜单点击:把 profile id 发到前端,前端切换编辑器内容
                            let profile_id = id.trim_start_matches("profile:").to_string();
                            let _ = app.emit("tray-select-profile", profile_id);
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            // 首次构建托盘菜单(含 hosts 子菜单)
            rebuild_tray_menu(app.handle());

            let window_clone = window.clone();
            let app_handle = app.handle().clone();
            window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    let ctx = storage::Context::Tauri(&app_handle);
                    if let Ok(config) = storage::load_config_internal(&ctx) {
                        if config.remember_close_choice {
                            if config.close_behavior == "tray" {
                                let _ = window_clone.hide();
                                api.prevent_close();
                            } else {
                                app_handle.exit(0);
                            }
                            return;
                        }
                    }
                    let _ = window_clone.emit("show-close-dialog", ());
                    api.prevent_close();
                }
            });

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    storage::check_auto_updates(&handle);
                }
            });

            // === WebDAV sync scheduler ===
            // - Reactive debounce: local mutations call schedule_sync, which sets a
            //   5s deadline. The scheduler loop below fires the sync when the
            //   deadline elapses.
            // - Startup pull: 3s after launch, do an initial pull so the device
            //   starts with the latest remote state.
            // - Periodic pull: every 5 minutes, re-pull to catch any missed changes.
            let scheduler = webdav::SyncScheduler::new(app.handle().clone());
            webdav::init_scheduler(scheduler.clone());
            // Run the debounce loop forever
            let sched_for_loop = scheduler.clone();
            tauri::async_runtime::spawn(async move {
                sched_for_loop.run_loop().await;
            });
            // Startup pull: 3s delay then pull
            let app_for_startup = app.handle().clone();
            let sched_for_startup = scheduler.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                match sched_for_startup.run_immediate().await {
                    Ok(Some(_)) => {} // success, status already updated
                    Ok(None) => {}    // skipped (not configured) — silent
                    Err(e) => {
                        // Configured but failed — surface to user
                        use tauri::Emitter;
                        let _ = app_for_startup.emit("webdav-error", format!("启动同步失败: {}", e));
                    }
                }
            });
            // Periodic pull: every 5 min
            let app_for_periodic = app.handle().clone();
            let sched_for_periodic = scheduler.clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(5 * 60)).await;
                    match sched_for_periodic.run_immediate().await {
                        Ok(Some(_)) => {}
                        Ok(None) => {}
                        Err(e) => {
                            use tauri::Emitter;
                            let _ = app_for_periodic.emit("webdav-error", format!("周期同步失败: {}", e));
                        }
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            hosts::get_system_hosts,
            hosts::save_system_hosts,
            hosts::check_write_permission,
            hosts::hostly_open_url,
            storage::load_config,
            storage::load_common_config,
            storage::save_common_config,
            storage::list_profiles,
            storage::create_profile,
            storage::save_profile_content,
            storage::delete_profile,
            storage::rename_profile,
            storage::toggle_profile_active,
            storage::set_multi_select,
            storage::apply_config,
            storage::import_file,
            storage::export_file,
            storage::import_data,
            storage::export_data,
            storage::import_switchhosts,
            storage::update_remote_config,
            storage::trigger_profile_update,
            storage::set_theme,
            storage::save_window_config,
            storage::save_sidebar_config,
            storage::set_auto_start,
            storage::get_auto_start,
            storage::save_close_behavior,
            storage::get_close_behavior,
            storage::save_remember_close_choice,
            storage::get_remember_close_choice,
            save_webdav_config,
            test_webdav_connection,
            sync_now,
            get_sync_status,
            show_main_window,
            hide_to_tray,
            exit_app,
            get_app_version,
            rebuild_tray_menu_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
fn show_main_window(window: tauri::Window) {
    window.show().unwrap();
    window.set_focus().unwrap();
}

#[tauri::command]
fn get_app_version(app: tauri::AppHandle) -> String {
    app.package_info().version.to_string()
}

/// Rebuild the tray menu with a fresh "Hosts" submenu listing all profiles.
/// Called at startup and whenever the profile list changes (frontend triggers
/// via the `rebuild_tray_menu` command).
fn rebuild_tray_menu(app: &tauri::AppHandle) {
    let tray = match app.tray_by_id("main") {
        Some(t) => t,
        None => return,
    };

    // Load profile list. On error, fall back to an empty submenu.
    let ctx = storage::Context::Tauri(app);
    let profiles: Vec<(String, String)> = match storage::load_config_internal(&ctx) {
        Ok(cfg) => cfg
            .profiles
            .into_iter()
            .map(|p| (p.id, p.name))
            .collect(),
        Err(_) => Vec::new(),
    };

    // Build the hosts submenu with SubmenuBuilder
    let mut builder = SubmenuBuilder::new(app, "快捷选择 hosts");
    if profiles.is_empty() {
        if let Ok(item) = MenuItem::with_id(app, "profile:empty", "（暂无配置）", false, None::<&str>) {
            builder = builder.item(&item);
        }
    } else {
        for (id, name) in &profiles {
            let label = if name.is_empty() { id.as_str() } else { name.as_str() };
            if let Ok(item) = MenuItem::with_id(
                app,
                format!("profile:{}", id),
                label,
                true,
                None::<&str>,
            ) {
                builder = builder.item(&item);
            }
        }
    }
    let hosts_submenu = match builder.build() {
        Ok(s) => s,
        Err(_) => return,
    };

    // Build the full tray menu
    let show_item = match MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>) {
        Ok(i) => i,
        Err(_) => return,
    };
    let quit_item = match MenuItem::with_id(app, "quit", "退出", true, None::<&str>) {
        Ok(i) => i,
        Err(_) => return,
    };
    let sep1 = match PredefinedMenuItem::separator(app) {
        Ok(i) => i,
        Err(_) => return,
    };
    let sep2 = match PredefinedMenuItem::separator(app) {
        Ok(i) => i,
        Err(_) => return,
    };

    let menu = match Menu::with_items(
        app,
        &[&show_item, &sep1, &hosts_submenu, &sep2, &quit_item],
    ) {
        Ok(m) => m,
        Err(_) => return,
    };

    let _ = tray.set_menu(Some(menu));
}

#[tauri::command]
fn rebuild_tray_menu_cmd(app: tauri::AppHandle) {
    rebuild_tray_menu(&app);
}

#[tauri::command]
fn hide_to_tray(window: tauri::Window) {
    window.hide().unwrap();
}

#[tauri::command]
fn exit_app(app: tauri::AppHandle) {
    app.exit(0);
}

// ============================ WebDAV Sync ============================

#[tauri::command]
fn save_webdav_config(
    app: tauri::AppHandle,
    url: String,
    username: String,
    password: String,
) -> Result<(), String> {
    // Persist password in OS keychain
    webdav::save_credentials(&username, &password)?;
    // Persist URL + username in LocalConfig
    let ctx = storage::Context::Tauri(&app);
    let mut local = storage::load_local_config_internal(&ctx)?;
    local.webdav_url = if url.is_empty() { None } else { Some(url) };
    local.webdav_username = if username.is_empty() { None } else { Some(username) };
    // Clear status on config change
    local.webdav_last_status = None;
    storage::save_local_config_internal(&ctx, &local)
}

#[tauri::command]
fn test_webdav_connection(app: tauri::AppHandle) -> Result<String, String> {
    let ctx = storage::Context::Tauri(&app);
    let local = storage::load_local_config_internal(&ctx)?;
    if local.webdav_url.is_none() || local.webdav_username.is_none() {
        return Err("请先填写并点击「保存配置」".to_string());
    }
    let url = local.webdav_url.unwrap();
    let username = local.webdav_username.unwrap();
    let password = webdav::load_credentials(&username)?;
    let cfg = webdav::WebDavConfig { url, username };
    webdav::test_connection(&cfg, &password)
}

#[tauri::command]
fn sync_now(app: tauri::AppHandle) -> Result<Option<webdav::SyncResult>, String> {
    webdav::sync_now_internal(&app)
}

#[tauri::command]
fn get_sync_status(app: tauri::AppHandle) -> Result<webdav::SyncStatus, String> {
    let ctx = storage::Context::Tauri(&app);
    let local = storage::load_local_config_internal(&ctx)?;
    Ok(webdav::SyncStatus {
        configured: local.webdav_url.is_some() && local.webdav_username.is_some(),
        last_sync: local.webdav_last_sync.clone(),
        last_status: local.webdav_last_status.clone(),
        last_message: None,
        username: local.webdav_username.clone(),
        url: local.webdav_url.clone(),
    })
}