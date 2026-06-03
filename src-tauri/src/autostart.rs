// 全部交给 tauri-plugin-autostart,它已经按平台分流:
//   Windows -> HKCU\Software\Microsoft\Windows\CurrentVersion\Run 注册表
//   macOS   -> ~/Library/LaunchAgents/com.hostly.switcher.plist (LaunchAgent)
//   Linux   -> ~/.config/autostart/hostly.desktop (XDG autostart)
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
