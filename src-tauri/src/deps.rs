use serde::Serialize;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const NODE_MIN: (u32, u32, u32) = (22, 19, 0);
const PI_PKG: &str = "@earendil-works/pi-coding-agent";
const WEB_PKG: &str = "pi-web-simple";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DepItem {
    pub id: String,
    pub title: String,
    pub ok: bool,
    pub detail: String,
    pub required: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DepsStatus {
    pub ready: bool,
    pub items: Vec<DepItem>,
    pub missing: Vec<String>,
    pub message: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DepsInstallResult {
    pub status: DepsStatus,
    pub log: String,
}

fn hide(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    {
        let _ = cmd;
    }
}

fn home_dir() -> PathBuf {
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

fn refresh_path() {
    #[cfg(windows)]
    {
        let mut parts = Vec::new();
        for key in [
            r"HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
            r"HKEY_CURRENT_USER\Environment",
        ] {
            if let Ok((code, out)) = run_capture("reg", &["query", key, "/v", "Path"]) {
                if code != 0 {
                    continue;
                }
                for line in out.lines() {
                    let line = line.trim();
                    if let Some(idx) = line.find("REG_EXPAND_SZ").or_else(|| line.find("REG_SZ")) {
                        let value = line[idx..]
                            .split_whitespace()
                            .nth(1)
                            .unwrap_or("")
                            .to_string();
                        if !value.is_empty() {
                            parts.push(value);
                        }
                    }
                }
            }
        }
        if !parts.is_empty() {
            let joined = parts.join(";");
            // Expand common %VAR% fragments for node installers.
            let expanded = joined.replace("%USERPROFILE%", &home_dir().display().to_string());
            unsafe {
                std::env::set_var("PATH", expanded);
            }
        }
    }
}

fn path_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    // ponytail: common local prefixes on this machine. Upgrade path: settings file.
    dirs.push(PathBuf::from(r"C:\dev\volta\bin"));
    dirs.push(PathBuf::from(r"C:\dev\npm-global"));
    dirs.push(PathBuf::from(r"C:\Program Files\nodejs"));
    dirs.push(home_dir().join(".pi").join("agent").join("bin"));
    dirs
}

fn find_on_path(names: &[&str]) -> Option<PathBuf> {
    for dir in path_dirs() {
        for name in names {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn run_capture(program: &str, args: &[&str]) -> Result<(i32, String), String> {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide(&mut cmd);
    let output = cmd
        .output()
        .map_err(|err| format!("无法执行 {program}：{err}"))?;
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    let err = String::from_utf8_lossy(&output.stderr);
    if !err.trim().is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(err.trim());
    }
    Ok((output.status.code().unwrap_or(-1), text))
}

fn parse_semver(raw: &str) -> Option<(u32, u32, u32)> {
    let trimmed = raw.trim().trim_start_matches('v');
    let mut parts = trimmed
        .split(|c: char| !c.is_ascii_digit())
        .filter(|p| !p.is_empty());
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

fn version_ok(raw: &str, min: (u32, u32, u32)) -> bool {
    parse_semver(raw).map(|v| v >= min).unwrap_or(false)
}

fn node_status() -> DepItem {
    match find_on_path(&["node.exe", "node"]) {
        Some(path) => match run_capture(&path.display().to_string(), &["--version"]) {
            Ok((0, out)) => {
                let ver = out.lines().next().unwrap_or("").trim().to_string();
                let ok = version_ok(&ver, NODE_MIN);
                DepItem {
                    id: "node".into(),
                    title: "Node.js".into(),
                    ok,
                    detail: if ok {
                        format!("{ver} · {}", path.display())
                    } else {
                        format!("{ver}（需要 ≥ 22.19.0）· {}", path.display())
                    },
                    required: true,
                }
            }
            Ok((_, out)) => DepItem {
                id: "node".into(),
                title: "Node.js".into(),
                ok: false,
                detail: format!("找到但无法读版本：{}", out.lines().next().unwrap_or("未知错误")),
                required: true,
            },
            Err(err) => DepItem {
                id: "node".into(),
                title: "Node.js".into(),
                ok: false,
                detail: err,
                required: true,
            },
        },
        None => DepItem {
            id: "node".into(),
            title: "Node.js".into(),
            ok: false,
            detail: "未安装（需要 ≥ 22.19.0）".into(),
            required: true,
        },
    }
}

fn bin_status(id: &str, title: &str, names: &[&str]) -> DepItem {
    // ponytail: do not exec pi/--version here; pi may open interactive UI and hang.
    match find_on_path(names) {
        Some(path) => DepItem {
            id: id.into(),
            title: title.into(),
            ok: true,
            detail: format!("已找到 · {}", path.display()),
            required: true,
        },
        None => DepItem {
            id: id.into(),
            title: title.into(),
            ok: false,
            detail: "未安装".into(),
            required: true,
        },
    }
}

pub fn status() -> DepsStatus {
    let items = vec![
        node_status(),
        bin_status("pi", "Pi", &["pi.cmd", "pi.exe", "pi"]),
        bin_status(
            "pi-web-simple",
            "Pi Web UI",
            &["pi-web-simple.cmd", "pi-web-simple.exe", "pi-web-simple"],
        ),
    ];
    let missing: Vec<String> = items
        .iter()
        .filter(|item| !item.ok)
        .map(|item| item.id.clone())
        .collect();
    let ready = missing.is_empty();
    let message = if ready {
        "依赖已就绪。".into()
    } else {
        let names = items
            .iter()
            .filter(|item| !item.ok)
            .map(|item| item.title.as_str())
            .collect::<Vec<_>>()
            .join(" / ");
        format!("缺少：{names}。点下方按钮确认后安装。")
    };
    DepsStatus {
        ready,
        items,
        missing,
        message,
    }
}

fn npm_cmd() -> Result<String, String> {
    find_on_path(&["npm.cmd", "npm.exe", "npm"])
        .map(|p| p.display().to_string())
        .ok_or_else(|| "找不到 npm，请先装好 Node.js。".to_string())
}

fn install_node(log: &mut String) -> Result<(), String> {
    log.push_str("> 安装 Node.js（winget）\n");
    match run_capture(
        "winget",
        &[
            "install",
            "-e",
            "--id",
            "OpenJS.NodeJS.LTS",
            "--accept-package-agreements",
            "--accept-source-agreements",
        ],
    ) {
        Ok((0, out)) => {
            log.push_str(&out);
            log.push('\n');
            Ok(())
        }
        Ok((code, out)) => {
            log.push_str(&out);
            log.push('\n');
            // Fallback: open official download page; user finishes MSI.
            let _ = open_url("https://nodejs.org/en/download");
            Err(format!(
                "winget 安装 Node.js 失败（退出码 {code}）。已打开官网下载页。"
            ))
        }
        Err(err) => {
            let _ = open_url("https://nodejs.org/en/download");
            Err(format!("{err}。已打开官网下载页。"))
        }
    }
}

fn open_url(url: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", "start", "", url])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        hide(&mut cmd);
        cmd.status()
            .map_err(|err| format!("打开链接失败：{err}"))?;
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(url)
            .status()
            .map_err(|err| format!("打开链接失败：{err}"))?;
        return Ok(());
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(url)
            .status()
            .map_err(|err| format!("打开链接失败：{err}"))?;
        return Ok(());
    }
}

fn npm_install_global(pkg: &str, extra: &[&str], log: &mut String) -> Result<(), String> {
    let npm = npm_cmd()?;
    log.push_str(&format!("> npm install -g {pkg}\n"));
    let mut argv = vec!["install", "-g", pkg];
    argv.extend_from_slice(extra);
    let (code, out) = if cfg!(windows) {
        // .cmd shims need cmd.exe; pass args separately to avoid quoting bugs.
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg(&npm).args(&argv);
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        hide(&mut cmd);
        let output = cmd
            .output()
            .map_err(|err| format!("无法执行 npm：{err}"))?;
        let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
        let err = String::from_utf8_lossy(&output.stderr);
        if !err.trim().is_empty() {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(err.trim());
        }
        (output.status.code().unwrap_or(-1), text)
    } else {
        run_capture(&npm, &argv)?
    };
    log.push_str(&out);
    log.push('\n');
    if code == 0 {
        Ok(())
    } else {
        Err(format!("安装 {pkg} 失败，退出码 {code}"))
    }
}

pub fn install_missing() -> Result<DepsInstallResult, String> {
    let before = status();
    if before.ready {
        return Ok(DepsInstallResult {
            status: before,
            log: "没有缺失项。".into(),
        });
    }

    let mut log = String::new();
    let mut errors = Vec::new();

    if before.missing.iter().any(|id| id == "node") {
        if let Err(err) = install_node(&mut log) {
            errors.push(err);
        }
        refresh_path();
    }

    let mid = status();
    if !mid.items.iter().any(|item| item.id == "node" && item.ok) {
        errors.push("Node.js 仍未就绪，跳过 pi / pi-web-simple。".into());
    } else {
        if mid.missing.iter().any(|id| id == "pi") {
            if let Err(err) = npm_install_global(PI_PKG, &["--ignore-scripts"], &mut log) {
                errors.push(err);
            }
        }
        let after_pi = status();
        if after_pi.missing.iter().any(|id| id == "pi-web-simple") {
            if let Err(err) = npm_install_global(WEB_PKG, &[], &mut log) {
                errors.push(err);
            }
        }
    }

    let after = status();
    if !errors.is_empty() && !after.ready {
        return Err(format!(
            "{}\n——\n{}",
            errors.join("\n"),
            after.message
        ));
    }
    Ok(DepsInstallResult {
        status: after,
        log,
    })
}

#[cfg(test)]
mod tests {
    use super::{parse_semver, status, version_ok};

    #[test]
    fn parses_node_version() {
        assert_eq!(parse_semver("v22.19.0"), Some((22, 19, 0)));
        assert!(version_ok("v24.9.0", (22, 19, 0)));
        assert!(!version_ok("v20.11.0", (22, 19, 0)));
    }

    #[test]
    fn status_lists_three_deps() {
        let st = status();
        assert_eq!(st.items.len(), 3);
        assert!(st.items.iter().any(|i| i.id == "node"));
        assert!(st.items.iter().any(|i| i.id == "pi"));
        assert!(st.items.iter().any(|i| i.id == "pi-web-simple"));
    }
}
