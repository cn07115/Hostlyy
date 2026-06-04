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
    menu::{Menu, MenuItem, PredefinedMenuItem, SubmenuBuilder, CheckMenuItem},
};
use std::io::Write;

/// 把 stderr 日志同时 append 到 `app_data_dir/hostlyy-tray.log`。
/// Windows GUI app (windows_subsystem = "windows") 没 console, stderr 看不到,
/// user 报告 "托盘切换没写入 hosts" 这种 IO 错误时根本没线索。
/// 写文件后 user 可在 `%APPDATA%\com.hostly.switcher\hostlyy-tray.log` 看。
fn log_tray(app: &tauri::AppHandle, msg: &str) {
    eprintln!("{}", msg);
    if let Ok(dir) = app.path().app_data_dir() {
        let _ = std::fs::create_dir_all(&dir);
        let log_path = dir.join("hostlyy-tray.log");
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
            let _ = writeln!(f, "[{}] {}", ts, msg);
        }
    }
}

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
        .plugin(tauri_plugin_notification::init())
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

            // 显式请求系统通知权限(Windows 10/11 第一次会弹 "是否允许此应用发送通知"
            // 对话框, 用户选"是"永久有效; macOS 第一次走系统设置里授权)。
            // 不调的话 tauri-plugin-notification 在某些 Windows 10 旧版上会静默失败
            // (show() 返回 Ok 但实际 toast 不出, 用户感知是"通知不弹")。
            {
                use tauri_plugin_notification::NotificationExt;
                match app.notification().request_permission() {
                    Ok(state) => {
                        use tauri_plugin_notification::PermissionState;
                        match state {
                            PermissionState::Granted => log_tray(&app.handle(), "[startup] ✓ 系统通知权限已授权"),
                            PermissionState::Denied => log_tray(&app.handle(), "[startup] ⚠️ 系统通知权限被拒 (到系统设置 → 系统 → 通知 → Hostlyy 打开)"),
                            PermissionState::Prompt => log_tray(&app.handle(), "[startup] ℹ️ 系统通知权限待用户确认"),
                            PermissionState::PromptWithRationale => log_tray(&app.handle(), "[startup] ℹ️ 系统通知需要 rationale (Android only)"),
                        }
                    }
                    Err(e) => log_tray(&app.handle(), &format!("[startup] 请求通知权限失败: {}", e)),
                }
            }

            let show_item = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            let _tray_handle = TrayIconBuilder::with_id("main")
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
                            // 托盘子菜单点击:在 Rust 端直接 toggle 该 profile 的 active 状态
                            // (走 multi_select 规则: 多选 toggle, 单选 设为唯一 active / 再点关掉,
                            // 跟前端主界面 checkbox 行为完全一致)。
                            //
                            // **不弹主窗口**:之前 window.show() + set_focus() 会把主窗口抢到前台,
                            // user 要的反馈是 "从托盘弹消息" (系统通知), 不是抢主窗口到屏幕中央。
                            //
                            // **不发 tray-select-profile 事件**:前端不再需要 toggle / open editor,
                            // 只需要在主窗口已经开的情况下刷新 sidebar profile 列表。
                            let profile_id = id.trim_start_matches("profile:").to_string();

                            // 拿 profile 名字用于通知(优先 name, 空就用 id)
                            let name = {
                                let ctx = storage::Context::Tauri(app);
                                storage::load_config_internal(&ctx)
                                    .ok()
                                    .and_then(|cfg| cfg.profiles.into_iter().find(|p| p.id == profile_id))
                                    .map(|p| if p.name.is_empty() { p.id.clone() } else { p.name })
                                    .unwrap_or_else(|| profile_id.clone())
                            };

                            // 调 toggle (它内部已经会 rebuild_tray_menu 同步 ✓ 标记 + apply_config 写 hosts)
                            log_tray(&app, &format!("[tray-click] profile_id={} name={} — 调 toggle_profile_active", profile_id, name));
                            match storage::toggle_profile_active(app.clone(), profile_id.clone()) {
                                Ok(()) => {
                                    log_tray(&app, &format!("[tray-click] toggle 成功, 应该已写 hosts + 同步托盘 ✓"));
                                }
                                Err(e) => {
                                    log_tray(&app, &format!("[tray-click] ⚠️ toggle 失败: {} (该错误一般因为 save_system_hosts 写 hosts 文件失败, 通常是 Windows 文件被锁定或没权限)", e));
                                }
                            }

                            // 读 toggle 后的新 active 状态, 文案区分 "已启用" / "已禁用"
                            // (toggle_profile_active_internal 已经更新 config, 读磁盘拿新 state)
                            let new_active = {
                                let ctx = storage::Context::Tauri(app);
                                storage::load_config_internal(&ctx)
                                    .ok()
                                    .and_then(|cfg| cfg.profiles.into_iter().find(|p| p.id == profile_id))
                                    .map(|p| p.active)
                                    .unwrap_or(false)
                            };
                            let action = if new_active { "已启用" } else { "已禁用" };

                            // 发系统通知 (Windows: WinRT toast, macOS: NSUserNotification,
                            // Linux: libnotify),不依赖主窗口可见。
                            // **关于停留时间**: tauri-plugin-notification 没暴露 duration API,
                            // Windows toast 默认 5s (系统设置 → 辅助功能 → 视觉效果 → 通知
                            // 持续时间可调, 但 Windows 11 不支持短于 5s)。
                            use tauri_plugin_notification::NotificationExt;
                            if let Err(e) = app
                                .notification()
                                .builder()
                                .title("Hostlyy")
                                .body(format!("{} {}", action, name))
                                .show()
                            {
                                log_tray(&app, &format!("[tray-click] ⚠️ 系统通知发送失败: {} (Windows: 检查设置 → 系统 → 通知 → Hostlyy 是否打开)", e));
                            }

                            // 发个事件给前端,如果主窗口正好开着,sidebar profile 列表刷新一下
                            let _ = app.emit("tray-select-profile", profile_id);
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
            check_update_with_proxy,
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
pub fn rebuild_tray_menu(app: &tauri::AppHandle) {
    let tray = match app.tray_by_id("main") {
        Some(t) => t,
        None => return,
    };

    // Load profile list. 直接从 profiles[i].active derive active set,
    // 不要读 active_profile_ids (v1.3.5 之前根本没同步, 老 user 装 v1.3.6+
    // 启动时 active_profile_ids 为空 → 托盘一个 ✓ 都不显示)。
    // profiles[i].active 才是 source of truth (apply_config 用这个)。
    let ctx = storage::Context::Tauri(app);
    let profiles: Vec<(String, String, bool)> = match storage::load_config_internal(&ctx) {
        Ok(cfg) => cfg
            .profiles
            .into_iter()
            .map(|p| (p.id, p.name, p.active))
            .collect(),
        Err(_) => Vec::new(),
    };

    // Build the hosts submenu with SubmenuBuilder。active 项用 CheckMenuItem,
    // 让 Windows 渲染 native checkbox (✓ 在左边状态列, 文字在右) — 所有项
    // 状态列宽一致, 不会再有"inactive 项左列空一截"看着像 bug 的视觉。
    // 之前用 MenuItem + "[✓] " 文字前缀, Windows 给所有项保留状态列宽
    // (因为有 ✓ 字符), 但 inactive 项的列是空的, 看着别扭。
    let mut builder = SubmenuBuilder::new(app, "快捷选择 hosts");
    if profiles.is_empty() {
        if let Ok(item) = MenuItem::with_id(app, "profile:empty", "（暂无配置）", false, None::<&str>) {
            builder = builder.item(&item);
        }
    } else {
        for (id, name, active) in &profiles {
            let base_label = if name.is_empty() { id.as_str() } else { name.as_str() };
            if let Ok(item) = CheckMenuItem::with_id(
                app,
                format!("profile:{}", id),
                base_label,
                true,
                *active,
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

/// 走代理检查更新:用 minreq 拉 latest.json,sed 替换当前 OS 对应 platform 的 url
/// 走代理,返回 { version, url }。前端拿到 url 后调 hostly_open_url,让系统默认
/// 浏览器/下载工具接管。
///
/// **async + tokio::task::spawn_blocking**:minreq 是 sync 阻塞 IO,直接在这个
/// 函数里调会冻住 Tauri 主线程,导致检查更新期间整个 UI 不响应鼠标键盘
/// (最多 15s timeout)。把 minreq 调用丢到 tokio 的 blocking thread pool,
/// 主线程立即释放,UI 不卡。普通 `async fn` 不够 — Tauri async command 还是要
/// 等 await 完成,只是 command 本身能并发。必须 `spawn_blocking` 才能让主线程
/// 在 IO 期间继续处理其他 command (UI 渲染 / 托盘 click 等)。
#[tauri::command]
async fn check_update_with_proxy(proxy_base: String) -> Result<serde_json::Value, String> {
    tokio::task::spawn_blocking(move || check_update_with_proxy_blocking(&proxy_base))
        .await
        .map_err(|e| format!("代理检查更新后台任务 join 失败: {}", e))?
}

/// sync 版本的检查更新逻辑(被 check_update_with_proxy 通过 spawn_blocking 调用)。
/// 全部用 minreq 阻塞 IO,放在 tokio blocking thread pool 里跑,主线程不卡。
fn check_update_with_proxy_blocking(proxy_base: &str) -> Result<serde_json::Value, String> {
    let base = proxy_base.trim().trim_end_matches('/');
    if base.is_empty() {
        return Err("代理地址不能为空".to_string());
    }

    // 拼接 latest.json 的代理 URL(注意 base 和 https:// 之间必须有 / 分隔,
    // 否则 minreq 解析 URL 时把整个 gh.xmly.devhttps://... 当 host, DNS 解析失败 11001)
    let mut current_url = format!(
        "{}/https://github.com/cn07115/Hostlyy/releases/latest/download/latest.json",
        base
    );

    // gh.xmly.dev / kkgithub / ghproxy / ghfast 之类的代理,见到
    // releases/latest/download/latest.json 都会 302 跳到
    // releases/download/vX.Y.Z/latest.json(具体版本号 URL)。
    // minreq 默认会自动 follow redirect,但它把 Location: /https://... 这种
    // path-relative URL 错误当 absolute URL 解析,报"invalid protocol"。
    // 修法: 禁掉 minreq 自动 redirect,自己处理 302 (最多 3 次, 防循环)。
    let mut response = minreq::get(&current_url)
        .with_timeout(15)
        .with_max_redirects(0)
        .send()
        .map_err(|e| format!("拉取 latest.json 失败: {}", e))?;

    let mut redirects_left = 3;
    while (300..400).contains(&response.status_code) && redirects_left > 0 {
        let location = response
            .headers
            .get("location")
            .ok_or_else(|| {
                format!("收到 HTTP {} 但响应没有 Location 头", response.status_code)
            })?
            .to_string();

        current_url = resolve_redirect(&current_url, &location)
            .map_err(|e| format!("follow redirect 失败: {}", e))?;

        response = minreq::get(&current_url)
            .with_timeout(15)
            .with_max_redirects(0)
            .send()
            .map_err(|e| format!("follow redirect 失败: {}", e))?;
        redirects_left -= 1;
    }

    if !(200..300).contains(&response.status_code) {
        return Err(format!(
            "拉取 latest.json 失败: HTTP {} (final URL: {})",
            response.status_code, current_url
        ));
    }

    let body = response
        .as_str()
        .map_err(|e| format!("读取 latest.json 失败: {}", e))?
        .to_string();

    // 解析
    let mut json: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("解析 latest.json 失败: {}", e))?;

    // 当前 OS 对应的 platform key(Tauri 2 标准命名)
    let platform_key = match std::env::consts::OS {
        "windows" => "windows-x86_64",
        "macos" => "darwin-x86_64",
        "linux" => "linux-x86_64",
        other => return Err(format!("不支持的平台: {}", other)),
    };

    // 取出原始 url(用 .get_mut 走一遍,顺便做替换)
    let proxied_url = {
        let entry = json
            .get_mut("platforms")
            .and_then(|p| p.get_mut(platform_key))
            .ok_or_else(|| {
                format!("latest.json 缺少 platform {} 节点(可能架构不匹配)", platform_key)
            })?;

        let url = entry
            .get("url")
            .and_then(|u| u.as_str())
            .ok_or_else(|| "platform 节点缺少 url 字段".to_string())?
            .to_string();

        if url.contains("https://github.com/cn07115/Hostly") {
            url.replace(
                "https://github.com/cn07115/Hostly",
                &format!("{}/https://github.com/cn07115/Hostly", base),
            )
        } else {
            url
        }
    };

    // 取出 version
    let version = json
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    // 取出 notes(plain text;启动 modal 用来显示更新内容)
    let notes = json
        .get("notes")
        .and_then(|n| n.as_str())
        .unwrap_or("")
        .to_string();

    Ok(serde_json::json!({
        "version": version,
        "notes": notes,
        "url": proxied_url,
    }))
}

/// 解析 302 redirect: absolute URL → 直接用;path-relative (Location: /...) → 跟
/// current_url 同 origin 拼起来。gh.xmly.dev 走的就是 path-relative, minreq 不会自己
/// 处理这种,所以我们手动拼。
fn resolve_redirect(current: &str, location: &str) -> Result<String, String> {
    if location.starts_with("http://") || location.starts_with("https://") {
        return Ok(location.to_string());
    }
    if location.starts_with('/') {
        let scheme_end = current
            .find("://")
            .ok_or_else(|| format!("invalid current URL (no scheme): {}", current))?;
        let after_scheme = &current[scheme_end + 3..];
        let path_start = after_scheme.find('/');
        let origin = match path_start {
            Some(i) => &current[..scheme_end + 3 + i],
            None => current,
        };
        return Ok(format!("{}{}", origin, location));
    }
    Err(format!(
        "unsupported redirect target: {} (current: {})",
        location, current
    ))
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