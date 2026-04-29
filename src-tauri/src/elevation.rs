#[cfg(all(target_os = "windows", feature = "auto-elevation"))]
pub fn relaunch_as_admin_if_needed() {
    let output = std::process::Command::new("net")
        .arg("session")
        .output();

    let is_admin = match output {
        Ok(result) => result.status.success(),
        Err(_) => false,
    };

    if !is_admin {
        let current_exe = std::env::current_exe().unwrap();
        let args: Vec<String> = std::env::args().skip(1).collect();
        let args_str = args
            .iter()
            .map(|arg| {
                if arg.contains(' ') {
                    format!("\"{}\"", arg)
                } else {
                    arg.to_string()
                }
            })
            .collect::<Vec<String>>()
            .join(" ");

        let mut cmd = std::process::Command::new("powershell");
        cmd.arg("Start-Process");
        cmd.arg(current_exe);
        if !args_str.is_empty() {
            cmd.arg("-ArgumentList");
            cmd.arg(format!("'{}'", args_str));
        }
        cmd.arg("-Verb");
        cmd.arg("RunAs");

        if let Ok(status) = cmd.status() {
            if status.success() {
                std::process::exit(0);
            }
        }
    }
}

#[cfg(not(all(target_os = "windows", feature = "auto-elevation")))]
pub fn relaunch_as_admin_if_needed() {}
