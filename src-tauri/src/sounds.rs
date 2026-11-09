use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
#[cfg(not(windows))]
use std::process::{Command, Stdio};
use tauri::{AppHandle, Manager};

const EXTENSION_FILE: &str = "armor-notify.js";
const DEFAULT_VOICE_PACK: &str = "edge-zh-xiaoxiao";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundPackInfo {
    pub id: String,
    pub title: String,
    pub path: String,
    pub kind: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundClipInfo {
    pub id: String,
    pub text: String,
    pub path: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundStatus {
    pub enabled: bool,
    pub pack_id: String,
    pub packs: Vec<SoundPackInfo>,
    pub clips: Vec<SoundClipInfo>,
    pub plugin_installed: bool,
    pub plugin_message: String,
    pub config_path: String,
}

#[derive(Deserialize)]
struct CatalogEntry {
    id: String,
    title: String,
    file: String,
    #[serde(default)]
    kind: Option<String>,
}

#[derive(Deserialize)]
struct PackManifest {
    #[serde(default)]
    clips: Vec<PackClip>,
}

#[derive(Deserialize)]
struct PackClip {
    id: String,
    text: String,
    file: String,
}

#[derive(Deserialize, Serialize)]
struct ArmorNotifyConfig {
    enabled: bool,
    #[serde(rename = "packId")]
    pack_id: String,
    dir: String,
}

fn agent_dir() -> PathBuf {
    dirs_home().join(".pi").join("agent")
}

fn dirs_home() -> PathBuf {
    if let Ok(home) = std::env::var("USERPROFILE") {
        if !home.is_empty() {
            return PathBuf::from(home);
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() {
            return PathBuf::from(home);
        }
    }
    PathBuf::from(".")
}

fn config_path() -> PathBuf {
    agent_dir().join("armor-notify.json")
}

fn legacy_done_sound_path() -> PathBuf {
    agent_dir().join("done-sound.json")
}

fn extension_install_path() -> PathBuf {
    agent_dir().join("extensions").join(EXTENSION_FILE)
}

fn sounds_root(_app: &AppHandle) -> PathBuf {
    let _ = crate::content::ensure();
    let saved = crate::content::sounds_dir();
    if saved.join("catalog.json").is_file() {
        return saved;
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("sounds")
}

fn extensions_root(app: &AppHandle) -> PathBuf {
    let bundled = app
        .path()
        .resource_dir()
        .ok()
        .map(|root| root.join("resources").join("extensions"));
    if let Some(path) = bundled {
        if path.join(EXTENSION_FILE).is_file() {
            return path;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("extensions")
}

pub fn plugin_installed() -> bool {
    extension_install_path().is_file()
}

fn load_catalog(root: &Path) -> Result<Vec<CatalogEntry>, String> {
    let text = fs::read_to_string(root.join("catalog.json"))
        .map_err(|err| format!("读音色目录失败：{err}"))?;
    serde_json::from_str(&text).map_err(|err| format!("音色目录无效：{err}"))
}

fn pack_infos(root: &Path) -> Result<Vec<SoundPackInfo>, String> {
    let catalog = load_catalog(root)?;
    let mut out = Vec::with_capacity(catalog.len());
    for entry in catalog {
        let path = root.join(&entry.id).join(&entry.file);
        if !path.is_file() {
            continue;
        }
        out.push(SoundPackInfo {
            id: entry.id,
            title: entry.title,
            path: path.display().to_string(),
            kind: entry.kind.unwrap_or_else(|| "tone".into()),
        });
    }
    if out.is_empty() {
        return Err("没有可用的音色包。".into());
    }
    Ok(out)
}

fn load_clips(root: &Path, pack_id: &str) -> Vec<SoundClipInfo> {
    let pack_dir = root.join(pack_id);
    let manifest_path = pack_dir.join("pack.json");
    let Ok(text) = fs::read_to_string(&manifest_path) else {
        let fallback = pack_dir.join("done.wav");
        if fallback.is_file() {
            return vec![SoundClipInfo {
                id: "done".into(),
                text: "完成提示".into(),
                path: fallback.display().to_string(),
            }];
        }
        return Vec::new();
    };
    let Ok(manifest) = serde_json::from_str::<PackManifest>(&text) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for clip in manifest.clips {
        let path = pack_dir.join(&clip.file);
        if !path.is_file() {
            continue;
        }
        out.push(SoundClipInfo {
            id: clip.id,
            text: clip.text,
            path: path.display().to_string(),
        });
    }
    out
}

fn read_notify_config() -> ArmorNotifyConfig {
    match fs::read_to_string(config_path()) {
        Ok(text) => serde_json::from_str(&text).unwrap_or(ArmorNotifyConfig {
            enabled: false,
            pack_id: DEFAULT_VOICE_PACK.into(),
            dir: String::new(),
        }),
        Err(_) => ArmorNotifyConfig {
            enabled: false,
            pack_id: DEFAULT_VOICE_PACK.into(),
            dir: String::new(),
        },
    }
}

fn write_notify_config(cfg: &ArmorNotifyConfig) -> Result<(), String> {
    let dir = agent_dir();
    fs::create_dir_all(&dir).map_err(|err| format!("无法创建配置目录：{err}"))?;
    let text = serde_json::to_string_pretty(cfg).map_err(|err| format!("配置序列化失败：{err}"))?;
    let mut file = fs::File::create(config_path()).map_err(|err| format!("写入配置失败：{err}"))?;
    file.write_all(text.as_bytes())
        .map_err(|err| format!("写入配置失败：{err}"))?;
    file.write_all(b"\n").map_err(|err| format!("写入配置失败：{err}"))?;
    // Silence legacy single-file notify so it does not double-play with armor-notify.
    let legacy = serde_json::json!({ "enabled": false, "sound": "off" });
    let _ = fs::write(
        legacy_done_sound_path(),
        format!("{}\n", serde_json::to_string_pretty(&legacy).unwrap_or_else(|_| "{}".into())),
    );
    Ok(())
}

fn match_pack_id(packs: &[SoundPackInfo], preferred: &str) -> String {
    if packs.iter().any(|pack| pack.id == preferred) {
        return preferred.to_string();
    }
    if packs.iter().any(|pack| pack.id == DEFAULT_VOICE_PACK) {
        return DEFAULT_VOICE_PACK.into();
    }
    packs
        .first()
        .map(|p| p.id.clone())
        .unwrap_or_else(|| DEFAULT_VOICE_PACK.into())
}

fn pack_dir(pack: &SoundPackInfo) -> Result<PathBuf, String> {
    Path::new(&pack.path)
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "音色包路径无效。".to_string())
}

#[cfg(windows)]
fn play_wav_windows(path: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "winmm")]
    unsafe extern "system" {
        fn PlaySoundW(psz_sound: *const u16, hmod: isize, fdw_sound: u32) -> i32;
    }

    // SND_SYNC | SND_FILENAME | SND_NODEFAULT
    const FLAGS: u32 = 0x0000 | 0x0002_0000 | 0x0002;
    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let ok = unsafe { PlaySoundW(wide.as_ptr(), 0, FLAGS) };
    if ok == 0 {
        Err(format!("试听失败：系统无法播放 {}", path.display()))
    } else {
        Ok(())
    }
}

fn play_wav(path: &Path) -> Result<(), String> {
    if !path.is_file() {
        return Err(format!("音色文件不存在：{}", path.display()));
    }
    #[cfg(windows)]
    {
        return play_wav_windows(path);
    }
    #[cfg(target_os = "macos")]
    {
        let mut cmd = Command::new("afplay");
        cmd.arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let status = cmd.status().map_err(|err| format!("试听失败：{err}"))?;
        if status.success() {
            Ok(())
        } else {
            Err("试听失败。".into())
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let mut cmd = Command::new("sh");
        cmd.args([
            "-c",
            "paplay \"$1\" 2>/dev/null || aplay -q \"$1\" 2>/dev/null",
            "sh",
        ])
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
        let status = cmd.status().map_err(|err| format!("试听失败：{err}"))?;
        if status.success() {
            Ok(())
        } else {
            Err("试听失败。".into())
        }
    }
}

pub fn ensure_plugin(app: &AppHandle) -> Result<String, String> {
    let src = extensions_root(app).join(EXTENSION_FILE);
    if !src.is_file() {
        return Err(format!("缺少扩展源文件：{}", src.display()));
    }
    let dest_dir = agent_dir().join("extensions");
    fs::create_dir_all(&dest_dir).map_err(|err| format!("无法创建扩展目录：{err}"))?;
    let dest = extension_install_path();
    fs::copy(&src, &dest).map_err(|err| format!("安装 armor-notify 失败：{err}"))?;
    Ok(format!("已安装语音扩展到 {}", dest.display()))
}

fn apply_pack(app: &AppHandle, pack_id: &str, enabled: bool) -> Result<ArmorNotifyConfig, String> {
    let packs = pack_infos(&sounds_root(app))?;
    let id = match_pack_id(&packs, pack_id);
    let pack = packs
        .iter()
        .find(|item| item.id == id)
        .ok_or_else(|| format!("没有音色「{id}」。"))?;
    let dest_dir = pack_dir(pack)?;
    let cfg = ArmorNotifyConfig {
        enabled,
        pack_id: id,
        dir: dest_dir.display().to_string(),
    };
    write_notify_config(&cfg)?;
    Ok(cfg)
}

pub fn status(app: &AppHandle) -> Result<SoundStatus, String> {
    let root = sounds_root(app);
    let packs = pack_infos(&root)?;
    let cfg = read_notify_config();
    let pack_id = match_pack_id(&packs, &cfg.pack_id);
    let installed = plugin_installed();
    Ok(SoundStatus {
        enabled: cfg.enabled,
        pack_id: pack_id.clone(),
        clips: load_clips(&root, &pack_id),
        packs,
        plugin_installed: installed,
        plugin_message: if installed {
            "armor-notify 已就绪（Pi 事件语音）。".into()
        } else {
            "尚未安装 armor-notify。打开开关时会自动安装。".into()
        },
        config_path: config_path().display().to_string(),
    })
}

pub fn set_enabled(app: &AppHandle, enabled: bool) -> Result<SoundStatus, String> {
    if enabled {
        let _ = ensure_plugin(app)?;
        let packs = pack_infos(&sounds_root(app))?;
        let current = read_notify_config();
        let preferred = if current.pack_id.is_empty() {
            DEFAULT_VOICE_PACK.to_string()
        } else {
            current.pack_id
        };
        let pack_id = packs
            .iter()
            .find(|item| item.id == preferred && item.kind == "voice")
            .map(|item| item.id.clone())
            .or_else(|| {
                packs
                    .iter()
                    .find(|item| item.id == DEFAULT_VOICE_PACK)
                    .map(|item| item.id.clone())
            })
            .unwrap_or(preferred);
        apply_pack(app, &pack_id, true)?;
    } else {
        let mut cfg = read_notify_config();
        cfg.enabled = false;
        write_notify_config(&cfg)?;
    }
    status(app)
}

pub fn set_pack(app: &AppHandle, pack_id: String) -> Result<SoundStatus, String> {
    let _ = ensure_plugin(app)?;
    apply_pack(app, &pack_id, true)?;
    status(app)
}

pub fn preview(
    app: &AppHandle,
    pack_id: Option<String>,
    clip_id: Option<String>,
) -> Result<(), String> {
    let root = sounds_root(app);
    let packs = pack_infos(&root)?;
    let cfg = read_notify_config();
    let id = match_pack_id(
        &packs,
        &pack_id.unwrap_or_else(|| cfg.pack_id.clone()),
    );
    let packs_has = packs.iter().any(|item| item.id == id);
    if !packs_has {
        return Err(format!("没有音色「{id}」。"));
    }
    if let Some(clip) = clip_id.filter(|value| !value.is_empty()) {
        let clips = load_clips(&root, &id);
        let item = clips
            .iter()
            .find(|item| item.id == clip)
            .ok_or_else(|| format!("没有提示「{clip}」。"))?;
        return play_wav(Path::new(&item.path));
    }
    let pack = packs
        .iter()
        .find(|item| item.id == id)
        .ok_or_else(|| format!("没有音色「{id}」。"))?;
    play_wav(Path::new(&pack.path))
}

pub fn rebind(app: &AppHandle) -> Result<(), String> {
    let cfg = read_notify_config();
    if cfg.enabled {
        let pack_id = if cfg.pack_id.is_empty() {
            DEFAULT_VOICE_PACK.to_string()
        } else {
            cfg.pack_id
        };
        apply_pack(app, &pack_id, true)?;
    }
    Ok(())
}

pub fn bootstrap(app: &AppHandle) -> Result<SoundStatus, String> {
    let install_msg = match ensure_plugin(app) {
        Ok(msg) => msg,
        Err(err) => err,
    };
    let cfg = read_notify_config();
    if cfg.enabled {
        let pack_id = if cfg.pack_id.is_empty() {
            DEFAULT_VOICE_PACK.to_string()
        } else {
            cfg.pack_id
        };
        let _ = apply_pack(app, &pack_id, true);
    }
    let mut st = status(app)?;
    st.plugin_message = install_msg;
    Ok(st)
}
