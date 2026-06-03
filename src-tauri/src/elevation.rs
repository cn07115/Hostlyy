// 跨平台 auto-elevation 入口:
//   Windows -> IsUserAnAdmin + ShellExecuteExW(runas) —— UAC 原生弹窗
//   macOS   -> osascript do shell script with administrator privileges —— 系统原生认证框
//   Linux   -> pkexec(走 polkit,大多数桌面环境会弹 GTK/Qt 认证框);失败回退 sudo
//
// 全部不 spawn 任何控制台子系统进程,所以不会有 cmd 窗口闪的现象。

#[cfg(target_os = "windows")]
pub fn relaunch_as_admin_if_needed() {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "shell32")]
    extern "system" {
        fn IsUserAnAdmin() -> i32;
        fn ShellExecuteExW(param: *mut SHELLEXECUTEINFOW) -> i32;
    }

    #[repr(C)]
    #[allow(non_snake_case)]
    struct SHELLEXECUTEINFOW {
        cbSize: u32,
        fMask: u32,
        hwnd: *mut core::ffi::c_void,
        lpVerb: *const u16,
        lpFile: *const u16,
        lpParameters: *const u16,
        lpDirectory: *const u16,
        nShow: i32,
        hInstApp: *mut core::ffi::c_void,
        lpIDList: *mut core::ffi::c_void,
        lpClass: *const u16,
        hkeyClass: *mut core::ffi::c_void,
        dwHotKey: u32,
        hMonitor: *mut core::ffi::c_void,
        hProcess: *mut core::ffi::c_void,
    }

    const SEE_MASK_NOASYNC: u32 = 0x00000100;
    const SW_SHOWNORMAL: i32 = 1;

    unsafe {
        if IsUserAnAdmin() != 0 {
            return;
        }

        let current_exe = match std::env::current_exe() {
            Ok(p) => p,
            Err(_) => return,
        };
        let args: Vec<String> = std::env::args().skip(1).collect();
        let args_str = args.join(" ");

        let verb: Vec<u16> = "runas\0".encode_utf16().collect();
        let exe: Vec<u16> = current_exe
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let params: Vec<u16> = if args_str.is_empty() {
            Vec::new()
        } else {
            args_str.encode_utf16().chain(std::iter::once(0)).collect()
        };

        let mut info: SHELLEXECUTEINFOW = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOASYNC;
        info.lpVerb = verb.as_ptr();
        info.lpFile = exe.as_ptr();
        info.lpParameters = if params.is_empty() {
            std::ptr::null()
        } else {
            params.as_ptr()
        };
        info.nShow = SW_SHOWNORMAL;

        if ShellExecuteExW(&mut info) != 0 {
            std::process::exit(0);
        }
    }
}

#[cfg(target_os = "macos")]
pub fn relaunch_as_admin_if_needed() {
    use std::io::Write;
    use std::process::Command;

    let current_exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return,
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args_quoted: Vec<String> = args
        .iter()
        .map(|a| shell_escape_posix(a))
        .collect();

    // 写到一个临时 .applescript 文件,避免手工拼字符串的转义陷阱
    let script = format!(
        "do shell script \"exec {bin} {args}\" with administrator privileges with prompt \"Hostly needs administrator privileges to modify system hosts file.\"",
        bin = shell_escape_posix(&current_exe.to_string_lossy()),
        args = args_quoted.join(" "),
    );

    let mut tmp = match tempfile::Builder::new()
        .prefix("hostly-elevate-")
        .suffix(".applescript")
        .tempfile()
    {
        Ok(f) => f,
        Err(_) => return,
    };
    if let Err(_) = tmp.write_all(script.as_bytes()) {
        return;
    }
    if let Err(_) = tmp.flush() {
        return;
    }

    // osascript 是 GUI 应用,不会闪 cmd 窗口。退出码非 0 走 fallback sudo(罕见)
    let status = Command::new("osascript").arg(tmp.path()).status();
    match status {
        Ok(s) if s.success() => std::process::exit(0),
        _ => {}
    }

    // Fallback: 走 sudo(终端里弹密码,需要用户从终端启动才看到)
    let mut cmd = Command::new("sudo");
    cmd.arg(&current_exe);
    for a in &args {
        cmd.arg(a);
    }
    if let Ok(s) = cmd.status() {
        if s.success() {
            std::process::exit(0);
        }
    }
}

#[cfg(target_os = "linux")]
pub fn relaunch_as_admin_if_needed() {
    use std::process::Command;

    let current_exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return,
    };
    let args: Vec<String> = std::env::args().skip(1).collect();

    // 优先 pkexec:走 polkit 弹 GTK/Qt 认证框,体验最像 UAC
    let pkexec_result = {
        let mut cmd = Command::new("pkexec");
        cmd.arg(&current_exe);
        for a in &args {
            cmd.arg(a);
        }
        cmd.status()
    };

    match pkexec_result {
        Ok(s) if s.success() => std::process::exit(0),
        // 126 = 鉴权被拒,127 = 命令未找到 —— 都不致命,继续 fallback
        _ => {}
    }

    // Fallback: sudo(适用于没装 polkit 的环境,比如 Alpine、一些容器)
    let mut cmd = Command::new("sudo");
    cmd.arg(&current_exe);
    for a in &args {
        cmd.arg(a);
    }
    if let Ok(s) = cmd.status() {
        if s.success() {
            std::process::exit(0);
        }
    }
}

#[cfg(target_os = "macos")]
fn shell_escape_posix(s: &str) -> String {
    // POSIX shell 单引号包裹:把字符串里的 ' 替换成 '\''
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
pub fn relaunch_as_admin_if_needed() {
    // 其他平台:不主动提权
}
