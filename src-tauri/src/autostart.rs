// 平台分流:
//   Windows -> 绕过 tauri-plugin-autostart(2.5.1 有 bug: 只写
//              HKCU\Software\...\Explorer\StartupApproved\Run 跟踪表,
//              不真写 Run 主键,自启失败),直接用 winreg 写
//              HKCU\Software\Microsoft\Windows\CurrentVersion\Run\Hostlyy
//              = "<exe path>" --autostarted
//   macOS   -> tauri-plugin-autostart -> ~/Library/LaunchAgents/<bundle>.plist
//   Linux   -> tauri-plugin-autostart -> ~/.config/autostart/<bundle>.desktop
use tauri::AppHandle;

#[cfg(target_os = "windows")]
mod imp {
    use std::env;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE};
    use winreg::RegKey;

    // 注册表 value name: 用 productName "Hostlyy" 区分(其他 autostart 项用 GameViewer /
    // PalmInput / Apifox 之类的简单 name 不会冲突)。
    const RUN_KEY_PATH: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
    const VALUE_NAME: &str = "Hostlyy";
    // 跟 lib.rs `tauri_plugin_autostart::init` 传的 args 一致,启动时识别 autostart 来源
    const AUTOSTART_ARG: &str = "--autostarted";

    pub fn set_auto_start(enable: bool) -> Result<(), String> {
        crate::log_tray_str(&format!("[set_auto_start] enter, enable={}", enable));
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let run_key = hkcu
            .open_subkey_with_flags(RUN_KEY_PATH, KEY_SET_VALUE | KEY_QUERY_VALUE)
            .map_err(|e| {
                let msg = format!("打开 Run 键失败: {}", e);
                crate::log_tray_str(&format!("[set_auto_start] {}", msg));
                msg
            })?;
        crate::log_tray_str("[set_auto_start] Run 键打开 Ok");

        // 清掉 tauri-plugin-autostart 2.5.1 留下的 StartupApproved\Run\Hostlyy
        // 跟踪表 (值 = 02 00 00 ... = "user has disabled", 让 Windows 不启动它)。
        // 不清这个值的话, 即便 Run 主键写对了 Windows 也会跳过我们。
        cleanup_plugin_leftover();
        crate::log_tray_str("[set_auto_start] cleanup_plugin_leftover done");

        if enable {
            let exe = env::current_exe()
                .map_err(|e| {
                    let msg = format!("获取当前 exe 路径失败: {}", e);
                    crate::log_tray_str(&format!("[set_auto_start] {}", msg));
                    msg
                })?
                .to_string_lossy()
                .to_string();
            if exe.is_empty() {
                crate::log_tray_str("[set_auto_start] ERR: exe 路径空");
                return Err("当前 exe 路径为空".to_string());
            }
            // 路径可能有空格,加双引号包起来
            let value = format!("\"{}\" {}", exe, AUTOSTART_ARG);
            crate::log_tray_str(&format!("[set_auto_start] writing Run\\{} = {}", VALUE_NAME, value));
            run_key
                .set_value(VALUE_NAME, &value)
                .map_err(|e| {
                    let msg = format!("写入 Run\\{} 失败: {}", VALUE_NAME, e);
                    crate::log_tray_str(&format!("[set_auto_start] {}", msg));
                    msg
                })?;
            crate::log_tray_str("[set_auto_start] write Ok");
        } else {
            // 删除时如果 value 不存在,忽略错误(用户本来就没开)
            match run_key.delete_value(VALUE_NAME) {
                Ok(_) => crate::log_tray_str(&format!("[set_auto_start] deleted Run\\{}", VALUE_NAME)),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    crate::log_tray_str(&format!("[set_auto_start] Run\\{} 本来就不存在, noop", VALUE_NAME));
                }
                Err(e) => {
                    let msg = format!("删除 Run\\{} 失败: {}", VALUE_NAME, e);
                    crate::log_tray_str(&format!("[set_auto_start] {}", msg));
                    return Err(msg);
                }
            }
        }
        Ok(())
    }

    /// 删 HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run\Hostlyy
    /// (tauri-plugin-autostart 2.5.1 的 bug 残留)。该值二进制 = 02 00 00 ... 表示
    /// "user has disabled", 删掉后 Windows 用默认行为 (Run 主键里有就走)。
    fn cleanup_plugin_leftover() {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let path = "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run";
        // 用 KEY_SET_VALUE 写权限打开, 删 value
        if let Ok(key) = hkcu.open_subkey_with_flags(path, KEY_SET_VALUE) {
            // delete_value 在 key 不存在时返回 NotFound, 静默忽略
            let _ = key.delete_value(VALUE_NAME);
        }
    }

    pub fn is_auto_start_enabled() -> bool {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        match hkcu.open_subkey_with_flags(RUN_KEY_PATH, KEY_QUERY_VALUE) {
            Ok(run_key) => {
                match run_key.get_value::<String, _>(VALUE_NAME) {
                    Ok(v) => {
                        crate::log_tray_str(&format!("[is_auto_start_enabled] Run\\{} = {} → true", VALUE_NAME, v));
                        true
                    }
                    Err(e) => {
                        crate::log_tray_str(&format!("[is_auto_start_enabled] Run\\{} 不存在 (kind={:?}) → false", VALUE_NAME, e.kind()));
                        false
                    }
                }
            }
            Err(e) => {
                crate::log_tray_str(&format!("[is_auto_start_enabled] 打开 Run 键失败: {} → false", e));
                false
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use tauri::AppHandle;
    use tauri_plugin_autostart::ManagerExt;

    pub fn set_auto_start(app: &AppHandle, enable: bool) -> Result<(), String> {
        let manager = app.autolaunch();
        if enable {
            manager.enable().map_err(|e| e.to_string())
        } else {
            manager.disable().map_err(|e| e.to_string())
        }
    }

    pub fn is_auto_start_enabled(app: &AppHandle) -> bool {
        app.autolaunch().is_enabled().unwrap_or(false)
    }
}

pub fn set_auto_start(app: &AppHandle, enable: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let _ = app; // Windows 分支不需要 app(直接调 winreg)
        imp::set_auto_start(enable)
    }
    #[cfg(not(target_os = "windows"))]
    {
        imp::set_auto_start(app, enable)
    }
}

pub fn is_auto_start_enabled(app: &AppHandle) -> bool {
    #[cfg(target_os = "windows")]
    {
        let _ = app; // 同上
        imp::is_auto_start_enabled()
    }
    #[cfg(not(target_os = "windows"))]
    {
        imp::is_auto_start_enabled(app)
    }
}
