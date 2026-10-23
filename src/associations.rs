use std::path::Path;

use crate::state::VIDEO_EXTS;

/// 旧版统一 ProgId（卸载时清理）
const OLD_PROGID: &str = "rust-videos.file";

/// 旧版曾注册的图片后缀（卸载时清理）
const LEGACY_IMAGE_EXTS: &[&str] = &[
    "jpg", "jpeg", "jfif", "png", "gif", "bmp", "webp", "ico", "svg", "avif",
];

fn progid_for(ext: &str) -> String {
    format!("rust-videos.{}", ext.to_ascii_lowercase())
}

/// 资源管理器「类型」列：真实后缀名，如「MP4 文件」
fn friendly_type_name(ext: &str) -> String {
    format!("{} 文件", ext.to_ascii_uppercase())
}

#[cfg(windows)]
fn dunce_path(p: &Path) -> String {
    dunce::simplified(p).to_string_lossy().replace('/', "\\")
}

#[cfg(windows)]
fn notify_shell() {
    #[link(name = "shell32")]
    unsafe extern "system" {
        fn SHChangeNotify(event: i32, flags: u32, item1: usize, item2: usize);
    }
    unsafe {
        SHChangeNotify(0x0800_0000, 0, 0, 0);
    }
}

#[cfg(windows)]
pub fn register_associations() -> Result<(), String> {
    use winreg::enums::*;
    use winreg::RegKey;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe = dunce_path(&exe);
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let classes = hkcu
        .open_subkey_with_flags("Software\\Classes", KEY_WRITE)
        .map_err(|e| format!("无法写入注册表: {e}"))?;

    // 清掉旧统一 ProgId / 图片关联
    let _ = classes.delete_subkey_all(OLD_PROGID);
    for ext in LEGACY_IMAGE_EXTS {
        clear_ext_assoc(&classes, ext)?;
    }

    for ext in VIDEO_EXTS {
        let pid = progid_for(ext);
        let friendly = friendly_type_name(ext);
        let (prog, _) = classes.create_subkey(&pid).map_err(|e| e.to_string())?;
        prog.set_value("", &friendly).map_err(|e| e.to_string())?;
        let (cmd, _) = prog
            .create_subkey("shell\\open\\command")
            .map_err(|e| e.to_string())?;
        cmd.set_value("", &format!("\"{exe}\" \"%1\""))
            .map_err(|e| e.to_string())?;
        let (icon, _) = prog.create_subkey("DefaultIcon").map_err(|e| e.to_string())?;
        icon.set_value("", &format!("{exe},0"))
            .map_err(|e| e.to_string())?;

        let key_name = format!(".{ext}");
        let (ext_key, _) = classes.create_subkey(&key_name).map_err(|e| e.to_string())?;
        ext_key.set_value("", &pid).map_err(|e| e.to_string())?;
        let (owp, _) = ext_key
            .create_subkey("OpenWithProgids")
            .map_err(|e| e.to_string())?;
        owp.set_value(&pid, &"").map_err(|e| e.to_string())?;
        // 去掉旧 ProgId
        let _ = owp.delete_value(OLD_PROGID);
    }
    notify_shell();
    Ok(())
}

#[cfg(windows)]
fn clear_ext_assoc(classes: &winreg::RegKey, ext: &str) -> Result<(), String> {
    use winreg::enums::*;
    let key_name = format!(".{ext}");
    if let Ok(ext_key) = classes.open_subkey_with_flags(&key_name, KEY_READ | KEY_WRITE) {
        if let Ok(val) = ext_key.get_value::<String, _>("") {
            if val == OLD_PROGID || val.starts_with("rust-videos.") {
                let _ = ext_key.delete_value("");
            }
        }
        if let Ok(owp) = ext_key.open_subkey_with_flags("OpenWithProgids", KEY_WRITE) {
            let _ = owp.delete_value(OLD_PROGID);
            let _ = owp.delete_value(progid_for(ext));
        }
    }
    let _ = classes.delete_subkey_all(progid_for(ext));
    Ok(())
}

#[cfg(windows)]
pub fn unregister_associations() -> Result<(), String> {
    use winreg::enums::*;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let classes = hkcu
        .open_subkey_with_flags("Software\\Classes", KEY_WRITE)
        .map_err(|e| format!("无法写入注册表: {e}"))?;

    let _ = classes.delete_subkey_all(OLD_PROGID);
    for ext in LEGACY_IMAGE_EXTS.iter().chain(VIDEO_EXTS.iter()) {
        clear_ext_assoc(&classes, ext)?;
    }
    notify_shell();
    Ok(())
}

#[cfg(not(windows))]
pub fn register_associations() -> Result<(), String> {
    Err("仅支持 Windows".into())
}

#[cfg(not(windows))]
pub fn unregister_associations() -> Result<(), String> {
    Err("仅支持 Windows".into())
}

#[cfg(windows)]
pub fn reveal_in_explorer(path: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let display = match std::fs::canonicalize(path) {
        Ok(abs) => dunce_path(&abs),
        Err(_) => dunce_path(path),
    };
    if !Path::new(&display).exists() && !path.exists() {
        return Err(format!("文件不存在：{display}"));
    }
    let arg = format!("/select,\"{display}\"");
    std::process::Command::new("explorer.exe")
        .raw_arg(arg)
        .spawn()
        .map_err(|e| format!("无法打开资源管理器：{e}"))?;
    Ok(())
}

#[cfg(not(windows))]
pub fn reveal_in_explorer(path: &Path) -> Result<(), String> {
    let _ = path;
    Err("仅支持 Windows".into())
}
