use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const HOST: &str = "127.0.0.1";
const PORT: u16 = 5319;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchInfo {
    pub name: String,
    pub version: String,
    pub port: u16,
    pub open_browser: bool,
    pub command: String,
    pub command_ok: bool,
}

#[derive(Clone, Serialize)]
pub struct WebUiStatus {
    pub running: bool,
    pub url: String,
    pub owned: bool,
    pub message: String,
}

struct OwnedChild {
    pid: u32,
}

pub struct Launcher {
    child: Mutex<Option<OwnedChild>>,
}

impl Launcher {
    pub fn new() -> Self {
        Self {
            child: Mutex::new(None),
        }
    }

    pub fn stop_owned(&self) -> Result<(), String> {
        let owned = self.child.lock().expect("launcher lock").take();
        if let Some(child) = owned {
            kill_tree(child.pid)?;
        }
        Ok(())
    }
}

impl Default for Launcher {
    fn default() -> Self {
        Self::new()
    }
}

fn page_url() -> String {
    crate::proxy::page_url()
}

fn addr() -> SocketAddr {
    format!("{HOST}:{PORT}").parse().expect("port")
}

pub fn http_status_ok(buf: &[u8]) -> bool {
    buf.starts_with(b"HTTP/1.1 200") || buf.starts_with(b"HTTP/1.0 200")
}

fn probe() -> bool {
    let mut stream = match TcpStream::connect_timeout(&addr(), Duration::from_millis(400)) {
        Ok(stream) => stream,
        Err(_) => return false,
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    if stream
        .write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .is_err()
    {
        return false;
    }
    let mut buf = [0u8; 64];
    match stream.read(&mut buf) {
        Ok(n) if n > 0 => http_status_ok(&buf[..n]),
        _ => false,
    }
}

fn status(running: bool, owned: bool, message: impl Into<String>) -> WebUiStatus {
    WebUiStatus {
        running,
        url: page_url(),
        owned,
        message: message.into(),
    }
}

fn find_bin() -> Result<PathBuf, String> {
    let mut dirs = Vec::new();
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    // ponytail: this machine's Volta/npm prefixes. Upgrade path: a settings file.
    dirs.push(PathBuf::from(r"C:\dev\volta\bin"));
    dirs.push(PathBuf::from(r"C:\dev\npm-global"));
    for dir in dirs {
        for name in ["pi-web-simple.cmd", "pi-web-simple.exe", "pi-web-simple"] {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }
    Err("找不到 pi-web-simple。请先安装官方包，并确认命令在 PATH 里。".into())
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

fn spawn_server(bin: &PathBuf) -> Result<u32, String> {
    let mut cmd = if cfg!(windows) {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg(bin);
        cmd
    } else {
        let cmd = Command::new(bin);
        cmd
    };
    cmd.env("PI_WEB_SIMPLE_OPEN", "0")
        .env("PI_WEB_SIMPLE_PORT", PORT.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    hide(&mut cmd);
    let child = cmd
        .spawn()
        .map_err(|err| format!("启动失败：{err}"))?;
    Ok(child.id())
}

fn wait_ready(timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if probe() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    false
}

fn kill_tree(pid: u32) -> Result<(), String> {
    let mut cmd = if cfg!(windows) {
        let mut cmd = Command::new("taskkill");
        cmd.args(["/F", "/T", "/PID", &pid.to_string()]);
        cmd
    } else {
        let mut cmd = Command::new("kill");
        cmd.args(["-TERM", &pid.to_string()]);
        cmd
    };
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    hide(&mut cmd);
    let out = cmd.status().map_err(|err| format!("停止失败：{err}"))?;
    if out.success() {
        Ok(())
    } else {
        Err(format!("停止失败，退出码 {}", out.code().unwrap_or(-1)))
    }
}

fn listening_pid() -> Option<u32> {
    let output = Command::new("netstat")
        .args(["-ano", "-p", "tcp"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let needle = format!(":{PORT}");
    for line in text.lines() {
        let line = line.trim();
        if !line.contains("LISTENING") || !line.contains(&needle) {
            continue;
        }
        return line.split_whitespace().last()?.parse().ok();
    }
    None
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebUiSettings {
    pub available: bool,
    pub appearance: String,
    pub language: String,
    pub browser_notifications: bool,
    pub message: String,
}

fn http_exchange(request: &[u8]) -> Result<String, String> {
    let mut stream = TcpStream::connect_timeout(&addr(), Duration::from_millis(800))
        .map_err(|_| "Web UI 未启动，读不到设置。".to_string())?;
    let _ = stream.set_read_timeout(Some(Duration::from_millis(1200)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(1200)));
    stream
        .write_all(request)
        .map_err(|err| format!("请求设置失败：{err}"))?;
    let mut buf = Vec::new();
    let _ = stream.read_to_end(&mut buf);
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

fn http_body(text: &str) -> &str {
    text.split("\r\n\r\n").nth(1).unwrap_or("").trim()
}

fn settings_unavailable(message: impl Into<String>) -> WebUiSettings {
    WebUiSettings {
        available: false,
        appearance: "system".into(),
        language: "system".into(),
        browser_notifications: false,
        message: message.into(),
    }
}

pub fn web_settings() -> WebUiSettings {
    let text = match http_exchange(
        b"GET /api/settings HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
    ) {
        Ok(text) => text,
        Err(err) => return settings_unavailable(err),
    };
    let body = http_body(&text);
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return settings_unavailable("设置返回不是 JSON。");
    };
    WebUiSettings {
        available: true,
        appearance: value
            .get("appearance")
            .and_then(|v| v.as_str())
            .unwrap_or("system")
            .into(),
        language: value
            .get("language")
            .and_then(|v| v.as_str())
            .unwrap_or("system")
            .into(),
        browser_notifications: value
            .get("browserNotifications")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        message: "已读取 Web UI 设置。".into(),
    }
}

pub fn set_browser_notifications(enabled: bool) -> Result<WebUiSettings, String> {
    let body = format!(r#"{{"browserNotifications":{enabled}}}"#);
    let request = format!(
        "PUT /api/settings HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let text = http_exchange(request.as_bytes())?;
    let ok_status = text.lines().next().map(|line| line.contains(" 200")).unwrap_or(false);
    if !ok_status && !text.contains("\"browserNotifications\"") {
        return Err("写入网页通知开关失败。".into());
    }
    Ok(web_settings())
}

pub fn web_appearance() -> String {
    web_settings().appearance
}

// Pi that is already listening is shown as-is. This process does not take ownership of it.
pub fn current() -> WebUiStatus {
    if probe() {
        status(true, false, "已启动")
    } else {
        status(false, false, "Web UI 还没启动。点右上角图标开始。")
    }
}

pub fn info() -> LaunchInfo {
    let (command, command_ok) = match find_bin() {
        Ok(path) => (path.display().to_string(), true),
        Err(err) => (err, false),
    };
    LaunchInfo {
        name: "Pi Armor".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        port: PORT,
        open_browser: false,
        command,
        command_ok,
    }
}

pub fn start(launcher: &Launcher) -> Result<WebUiStatus, String> {
    if probe() {
        return Ok(status(true, false, "已启动"));
    }
    let bin = find_bin()?;
    let pid = spawn_server(&bin)?;
    *launcher.child.lock().expect("launcher lock") = Some(OwnedChild { pid });
    if wait_ready(Duration::from_secs(25)) {
        return Ok(status(true, true, "已启动"));
    }
    let _ = kill_tree(pid);
    *launcher.child.lock().expect("launcher lock") = None;
    Err("Web UI 启动超时。请在终端执行 pi-web-simple 看它能否自己起来。".into())
}

pub fn stop(launcher: &Launcher) -> Result<WebUiStatus, String> {
    let owned = launcher.child.lock().expect("launcher lock").take();
    match owned {
        Some(child) => {
            kill_tree(child.pid)?;
            Ok(status(probe(), false, "已停止"))
        }
        None => match listening_pid() {
            Some(pid) => {
                kill_tree(pid)?;
                Ok(status(probe(), false, "已停止"))
            }
            None => Ok(status(false, false, "已停止")),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::http_status_ok;

    #[test]
    fn accepts_http_200() {
        assert!(http_status_ok(b"HTTP/1.1 200 OK\r\n"));
        assert!(http_status_ok(b"HTTP/1.0 200 OK\r\n"));
        assert!(!http_status_ok(b"HTTP/1.1 404 Not Found"));
    }
}
