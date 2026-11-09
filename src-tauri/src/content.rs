use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const POINTER: &str = "resources-dir.txt";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentStatus {
    pub dir: String,
    pub themes_dir: String,
    pub sounds_dir: String,
    pub is_default: bool,
}

pub fn status() -> ContentStatus {
    let _ = ensure();
    let dir = dir();
    ContentStatus {
        dir: dir.display().to_string(),
        themes_dir: dir.join("themes").display().to_string(),
        sounds_dir: dir.join("sounds").display().to_string(),
        is_default: custom_dir().is_none(),
    }
}

pub fn dir() -> PathBuf {
    if let Some(path) = custom_dir() {
        return path;
    }
    default_dir()
}

pub fn themes_dir() -> PathBuf {
    dir().join("themes")
}

pub fn sounds_dir() -> PathBuf {
    dir().join("sounds")
}

pub fn set_dir(path: String) -> Result<String, String> {
    let target = PathBuf::from(path.trim());
    if !target.is_absolute() {
        return Err("请选择完整的文件夹路径。".into());
    }
    let current = dir();
    fs::create_dir_all(target.join("themes")).map_err(|err| format!("这个位置建不了文件夹：{err}"))?;
    fs::create_dir_all(target.join("sounds")).map_err(|err| format!("这个位置建不了文件夹：{err}"))?;
    if target != current {
        copy_tree(&current, &target, false)?;
    }
    write_pointer(&target)?;
    ensure_into(&target)?;
    Ok(target.display().to_string())
}

pub fn reset_dir() -> Result<String, String> {
    if let Some(custom) = custom_dir() {
        let fallback = default_dir();
        fs::create_dir_all(&fallback).map_err(|err| format!("无法恢复默认位置：{err}"))?;
        copy_tree(&custom, &fallback, true)?;
    }
    if let Some(path) = pointer_path() {
        let _ = fs::remove_file(path);
    }
    ensure_into(&default_dir())?;
    Ok(default_dir().display().to_string())
}

pub fn open_dir() -> Result<String, String> {
    let path = dir();
    fs::create_dir_all(&path).map_err(|err| format!("无法打开文件夹：{err}"))?;
    Command::new("explorer")
        .arg(&path)
        .spawn()
        .map_err(|err| format!("无法打开文件夹：{err}"))?;
    Ok(path.display().to_string())
}

pub fn pick_folder() -> Option<String> {
    rfd::FileDialog::new()
        .set_title("选择保存位置")
        .pick_folder()
        .map(|path| path.display().to_string())
}

pub fn ensure() -> Result<(), String> {
    ensure_into(&dir())
}

fn ensure_into(root: &Path) -> Result<(), String> {
    let themes = root.join("themes");
    let sounds = root.join("sounds");
    fs::create_dir_all(&themes).map_err(|err| format!("无法建立主题文件夹：{err}"))?;
    fs::create_dir_all(&sounds).map_err(|err| format!("无法建立音色文件夹：{err}"))?;
    copy_tree(&bundled_themes(), &themes, false)?;
    copy_tree(&bundled_sounds(), &sounds, false)?;
    if let Some(custom) = armor_dir().map(|dir| dir.join("web-theme.css")) {
        let target = themes.join("custom.css");
        if custom.is_file() && !target.exists() {
            let _ = fs::copy(custom, target);
        }
    }
    Ok(())
}

fn default_dir() -> PathBuf {
    documents_dir()
        .or_else(armor_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Pi Armor")
}

fn custom_dir() -> Option<PathBuf> {
    let raw = fs::read_to_string(pointer_path()?).ok()?;
    let path = PathBuf::from(raw.trim());
    path.is_dir().then_some(path)
}

fn write_pointer(target: &Path) -> Result<(), String> {
    let Some(dir) = armor_dir() else {
        return Err("找不到用户目录，位置没能保存。".into());
    };
    fs::create_dir_all(&dir).map_err(|err| format!("无法保存位置：{err}"))?;
    fs::write(dir.join(POINTER), target.display().to_string())
        .map_err(|err| format!("无法保存位置：{err}"))
}

fn pointer_path() -> Option<PathBuf> {
    Some(armor_dir()?.join(POINTER))
}

pub fn armor_dir() -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    Some(PathBuf::from(home).join(".pi").join("armor"))
}

fn bundled_themes() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("themes")
}

fn bundled_sounds() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("sounds")
}

fn copy_tree(from: &Path, to: &Path, overwrite: bool) -> Result<(), String> {
    if !from.is_dir() {
        return Ok(());
    }
    fs::create_dir_all(to).map_err(|err| format!("无法建立文件夹：{err}"))?;
    let read = fs::read_dir(from).map_err(|err| format!("无法读取保存内容：{err}"))?;
    for entry in read {
        let entry = entry.map_err(|err| format!("无法读取保存内容：{err}"))?;
        let src = entry.path();
        let dest = to.join(entry.file_name());
        if src.is_dir() {
            copy_tree(&src, &dest, overwrite)?;
            continue;
        }
        if !src.is_file() || (!overwrite && dest.exists()) {
            continue;
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|err| format!("无法建立文件夹：{err}"))?;
        }
        fs::copy(&src, &dest).map_err(|err| format!("无法放入保存内容：{err}"))?;
    }
    Ok(())
}

fn documents_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        #[repr(C)]
        struct Guid {
            data1: u32,
            data2: u16,
            data3: u16,
            data4: [u8; 8],
        }
        #[link(name = "shell32")]
        unsafe extern "system" {
            fn SHGetKnownFolderPath(
                rfid: *const Guid,
                flags: u32,
                token: *mut std::ffi::c_void,
                path: *mut *mut u16,
            ) -> i32;
        }
        #[link(name = "ole32")]
        unsafe extern "system" {
            fn CoTaskMemFree(pv: *mut std::ffi::c_void);
        }
        const FOLDERID_DOCUMENTS: Guid = Guid {
            data1: 0xFDD39AD0,
            data2: 0x238F,
            data3: 0x46AF,
            data4: [0xAD, 0xB4, 0x6C, 0x85, 0x48, 0x03, 0x69, 0xC7],
        };
        unsafe {
            let mut raw = std::ptr::null_mut();
            let hr = SHGetKnownFolderPath(&FOLDERID_DOCUMENTS, 0, std::ptr::null_mut(), &mut raw);
            if hr != 0 || raw.is_null() {
                return None;
            }
            let mut len = 0usize;
            while *raw.add(len) != 0 {
                len += 1;
            }
            let text = String::from_utf16_lossy(std::slice::from_raw_parts(raw, len));
            CoTaskMemFree(raw.cast());
            return Some(PathBuf::from(text));
        }
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Documents"))
    }
}
