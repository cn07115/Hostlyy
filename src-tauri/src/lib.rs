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
    menu::{Menu, MenuItem},
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostarted"]),
        ))
        .setup(|app| {
            if cli::run_cli(Some(&app.handle())) {
                std::process::exit(0);
            }

            let window = app.get_webview_window("main").unwrap();

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
                .tooltip("Hostly")
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
    let url = local.webdav_url.ok_or("WebDAV URL 未配置")?;
    let username = local.webdav_username.ok_or("WebDAV 用户名未配置")?;
    let password = webdav::load_credentials(&username)?;
    let cfg = webdav::WebDavConfig { url, username };
    webdav::test_connection(&cfg, &password)
}

#[tauri::command]
fn sync_now(app: tauri::AppHandle) -> Result<webdav::SyncResult, String> {
    let ctx = storage::Context::Tauri(&app);
    let mut local = storage::load_local_config_internal(&ctx)?;
    let url = local.webdav_url.clone().ok_or("WebDAV URL 未配置")?;
    let username = local.webdav_username.clone().ok_or("WebDAV 用户名未配置")?;
    let password = webdav::load_credentials(&username)?;
    let cfg = webdav::WebDavConfig { url, username };

    let app_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let result = webdav::perform_sync(&app_dir, &cfg, &password);

    // Update status regardless of success
    match &result {
        Ok(r) => {
            local.webdav_last_sync = Some(webdav::now_iso());
            if r.errors.is_empty() {
                local.webdav_last_status = Some("ok".to_string());
            } else {
                local.webdav_last_status = Some(format!("partial: {}", r.errors.len()));
            }
        }
        Err(e) => {
            local.webdav_last_sync = Some(webdav::now_iso());
            local.webdav_last_status = Some(format!("error: {}", e));
        }
    }
    let _ = storage::save_local_config_internal(&ctx, &local);

    result
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