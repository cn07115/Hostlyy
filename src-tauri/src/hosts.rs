use std::fs;
use std::path::PathBuf;

#[cfg(target_os = "windows")]
fn get_hosts_path() -> PathBuf {
    PathBuf::from("C:\\Windows\\System32\\drivers\\etc\\hosts")
}

#[cfg(not(target_os = "windows"))]
fn get_hosts_path() -> PathBuf {
    PathBuf::from("/etc/hosts")
}

#[tauri::command]
pub fn get_system_hosts() -> Result<String, String> {
    let path = get_hosts_path();
    fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_system_hosts(content: String) -> Result<(), String> {
    let path = get_hosts_path();
    
    // Attempt normal write first
    match fs::write(&path, &content) {
        Ok(_) => Ok(()),
        Err(e) => {
            #[cfg(target_os = "macos")]
            {
                let direct_err = e.to_string();
                println!("Direct write failed: {}. Attempting elevation...", direct_err);
                
                // Try elevation
                match save_hosts_elevated_macos(&content) {
                    Ok(_) => Ok(()),
                    Err(elevated_err) => {
                        // Return BOTH errors so we know what happened
                        Err(format!("Save failed. Direct: [{}]. Elevated: [{}]", direct_err, elevated_err))
                    }
                }
            }
            
            #[cfg(not(target_os = "macos"))]
            Err(e.to_string())
        }
    }
}

#[cfg(target_os = "macos")]
fn save_hosts_elevated_macos(content: &str) -> Result<(), String> {
    use std::io::Write;
    
    // Create a temporary file
    let mut temp_file = match tempfile::NamedTempFile::new() {
        Ok(t) => t,
        Err(e) => return Err(format!("TempFile creation failed: {}", e)),
    };
    
    if let Err(e) = write!(temp_file, "{}", content) {
        return Err(format!("TempFile write failed: {}", e));
    }
    
    let temp_path = temp_file.path().to_string_lossy().to_string();

    // Move temp file to /etc/hosts using authentication
    let script = format!(
        "do shell script \"mv -f '{}' /etc/hosts && chmod 644 /etc/hosts\" with administrator privileges",
        temp_path
    );

    let output = std::process::Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|e| format!("Osascript spawn failed: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(format!("Elevation script failed (code {:?}): {}", output.status.code(), String::from_utf8_lossy(&output.stderr)))
    }
}

#[tauri::command]
pub fn check_write_permission() -> Result<bool, String> {
    let path = get_hosts_path();
    // Try to open the file in append mode. This checks if we have write permissions 
    // without actually modifying or truncating the file.
    let result = std::fs::OpenOptions::new()
        .write(true)
        .append(true)
        .open(&path);
        
    Ok(result.is_ok())
}

#[tauri::command]
pub fn hostly_open_url(url: String) -> Result<(), String> {
    // Use the `open` crate: cross-platform, no flashing cmd window on Windows
    // (the previous implementation used `cmd /C start <url>` which briefly
    // allocated a console). Detaches the child so the GUI doesn't block.
    open::that_detached(&url).map_err(|e| e.to_string())
}
