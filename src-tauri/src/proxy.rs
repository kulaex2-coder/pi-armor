use serde::Serialize;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread;
use std::time::Duration;

const HOST: &str = "127.0.0.1";
const UPSTREAM_PORT: u16 = 5319;
const PROXY_PORT: u16 = 5320;
const THEME_PATH: &str = "/__pi_armor/theme.css";
const THEME_TAG: &str = "<link rel=\"stylesheet\" data-pi-armor=\"theme\" href=\"/__pi_armor/theme.css\">";
const HEADER_CAP: usize = 64 * 1024;
const BODY_CAP: usize = 8 * 1024 * 1024;

struct Proxy {
    port: u16,
    stop: Arc<AtomicBool>,
}

impl Drop for Proxy {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect_timeout(&loopback(self.port), Duration::from_millis(200));
    }
}

static PROXY: OnceLock<Proxy> = OnceLock::new();

pub fn page_url() -> String {
    let port = PROXY
        .get_or_init(|| {
            let upstream = loopback(UPSTREAM_PORT);
            start_proxy(upstream, Some(PROXY_PORT)).unwrap_or(Proxy {
                port: 0,
                stop: Arc::new(AtomicBool::new(true)),
            })
        })
        .port;
    if port == 0 {
        format!("http://{HOST}:{UPSTREAM_PORT}/")
    } else {
        format!("http://{HOST}:{port}/")
    }
}

fn loopback(port: u16) -> SocketAddr {
    format!("{HOST}:{port}").parse().expect("loopback")
}

fn start_proxy(upstream: SocketAddr, port: Option<u16>) -> std::io::Result<Proxy> {
    let listener = match port {
        Some(port) => TcpListener::bind(loopback(port))?,
        None => TcpListener::bind((HOST, 0))?,
    };
    let bound = listener.local_addr()?.port();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stop);
    thread::spawn(move || accept_loop(listener, upstream, bound, flag));
    Ok(Proxy { port: bound, stop })
}

fn accept_loop(listener: TcpListener, upstream: SocketAddr, proxy_port: u16, stop: Arc<AtomicBool>) {
    loop {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        match listener.accept() {
            Ok((stream, _)) => {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                thread::spawn(move || {
                    let _ = handle(stream, upstream, proxy_port);
                });
            }
            Err(_) => break,
        }
    }
}

fn handle(mut client: TcpStream, upstream: SocketAddr, proxy_port: u16) -> std::io::Result<()> {
    let _ = client.set_nodelay(true);
    let _ = client.set_read_timeout(Some(Duration::from_secs(20)));
    let _ = client.set_write_timeout(Some(Duration::from_secs(20)));

    let mut buf = Vec::new();
    let head_end = read_headers(&mut client, &mut buf)?;
    let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
    let Some((method, target)) = request_line(&head) else {
        return write_text(&mut client, 400, "bad request");
    };
    let path = target.split('?').next().unwrap_or(target);
    let body_len = content_length(&head).unwrap_or(0);
    if body_len > BODY_CAP {
        return write_text(&mut client, 413, "body too large");
    }
    let need = head_end + body_len;
    read_exact(&mut client, &mut buf, need)?;
    let body = buf[head_end..need].to_vec();

    if method == "GET" && path == THEME_PATH {
        return write_theme(&mut client);
    }

    let mut server = match TcpStream::connect_timeout(&upstream, Duration::from_secs(3)) {
        Ok(stream) => stream,
        Err(_) => return write_text(&mut client, 502, "Web UI 未启动"),
    };
    let _ = server.set_nodelay(true);
    let _ = server.set_read_timeout(Some(Duration::from_secs(20)));
    let _ = server.set_write_timeout(Some(Duration::from_secs(20)));
    server.write_all(&forward_request(&head, &body, upstream))?;
    server.flush()?;

    let mut raw = Vec::new();
    let response_end = read_headers(&mut server, &mut raw)?;
    let response_head = String::from_utf8_lossy(&raw[..response_end]).into_owned();
    let leftover = raw[response_end..].to_vec();
    let status = response_head.lines().next().unwrap_or("HTTP/1.1 502 Bad Gateway");

    if should_inject(method, &response_head) && method != "HEAD" {
        let payload = read_body(&mut server, &response_head, leftover)?;
        let text = String::from_utf8_lossy(&payload);
        let injected = inject_theme(&text);
        return write_response(
            &mut client,
            status,
            &response_head,
            proxy_port,
            Some(injected.as_bytes()),
        );
    }

    let _ = server.set_read_timeout(None);
    let _ = client.set_read_timeout(None);
    write_response(&mut client, status, &response_head, proxy_port, None)?;
    if method == "HEAD" {
        return Ok(());
    }
    if let Some(len) = content_length(&response_head) {
        let mut sent = leftover.len().min(len);
        client.write_all(&leftover[..sent])?;
        let mut tmp = [0u8; 8192];
        while sent < len {
            let want = (len - sent).min(tmp.len());
            let n = server.read(&mut tmp[..want])?;
            if n == 0 {
                break;
            }
            client.write_all(&tmp[..n])?;
            sent += n;
        }
        return Ok(());
    }
    client.write_all(&leftover)?;
    let mut tmp = [0u8; 8192];
    loop {
        match server.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => client.write_all(&tmp[..n])?,
            Err(_) => break,
        }
    }
    Ok(())
}

fn should_inject(method: &str, head: &str) -> bool {
    if method != "GET" && method != "HEAD" {
        return false;
    }
    if header(head, "content-encoding").is_some() {
        return false;
    }
    header(head, "content-type")
        .map(|value| value.to_ascii_lowercase().contains("text/html"))
        .unwrap_or(false)
}

pub fn inject_theme(html: &str) -> String {
    if html.contains("data-pi-armor=\"theme\"") {
        return html.to_string();
    }
    match html.rfind("</head>") {
        Some(index) => {
            let mut out = String::with_capacity(html.len() + THEME_TAG.len());
            out.push_str(&html[..index]);
            out.push_str(THEME_TAG);
            out.push_str(&html[index..]);
            out
        }
        None => html.to_string(),
    }
}

fn forward_request(head: &str, body: &[u8], upstream: SocketAddr) -> Vec<u8> {
    let mut lines = head.split("\r\n");
    let request = lines.next().unwrap_or("GET / HTTP/1.1");
    let mut out = format!(
        "{request}\r\nHost: {}:{}\r\nConnection: close\r\n",
        upstream.ip(),
        upstream.port()
    );
    for line in lines {
        if line.is_empty() {
            break;
        }
        let name = line.split(':').next().unwrap_or("");
        if matches!(
            name.to_ascii_lowercase().as_str(),
            "host" | "connection" | "proxy-connection" | "keep-alive" | "accept-encoding" | "te"
        ) {
            continue;
        }
        out.push_str(line);
        out.push_str("\r\n");
    }
    out.push_str("\r\n");
    let mut bytes = out.into_bytes();
    bytes.extend_from_slice(body);
    bytes
}

fn write_response(
    client: &mut TcpStream,
    status: &str,
    head: &str,
    proxy_port: u16,
    replace_body: Option<&[u8]>,
) -> std::io::Result<()> {
    let mut out = format!("{status}\r\n");
    for line in head.split("\r\n").skip(1) {
        if line.is_empty() {
            break;
        }
        let name = line.split(':').next().unwrap_or("").to_ascii_lowercase();
        if matches!(name.as_str(), "connection" | "keep-alive" | "proxy-connection") {
            continue;
        }
        if name == "transfer-encoding" || name == "content-length" {
            if replace_body.is_some() {
                continue;
            }
            if name == "content-length" && should_inject("HEAD", head) {
                if let Some(len) = content_length(head) {
                    out.push_str(&format!("Content-Length: {}\r\n", len + THEME_TAG.len()));
                    continue;
                }
            }
            out.push_str(line);
            out.push_str("\r\n");
            continue;
        }
        if name == "location" {
            let value = line.split_once(':').map(|(_, value)| value.trim()).unwrap_or("");
            let rewritten = value.replacen(
                &format!("http://{HOST}:{UPSTREAM_PORT}"),
                &format!("http://{HOST}:{proxy_port}"),
                1,
            );
            out.push_str("Location: ");
            out.push_str(&rewritten);
            out.push_str("\r\n");
            continue;
        }
        out.push_str(line);
        out.push_str("\r\n");
    }
    if let Some(body) = replace_body {
        out.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    out.push_str("Connection: close\r\n\r\n");
    client.write_all(out.as_bytes())?;
    if let Some(body) = replace_body {
        client.write_all(body)?;
    }
    client.flush()
}

fn write_theme(client: &mut TcpStream) -> std::io::Result<()> {
    let css = theme_css();
    let head = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/css; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
        css.len()
    );
    client.write_all(head.as_bytes())?;
    client.write_all(css.as_bytes())?;
    client.flush()
}

fn write_text(client: &mut TcpStream, code: u16, text: &str) -> std::io::Result<()> {
    let reason = match code {
        400 => "Bad Request",
        413 => "Payload Too Large",
        _ => "Bad Gateway",
    };
    let head = format!(
        "HTTP/1.1 {code} {reason}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        text.len()
    );
    client.write_all(head.as_bytes())?;
    client.write_all(text.as_bytes())?;
    client.flush()
}

#[derive(Clone, Serialize)]
pub struct ThemeItem {
    pub id: String,
    pub title: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeStatus {
    pub id: String,
    pub title: String,
    pub path: String,
    pub resource_dir: String,
    pub themes: Vec<ThemeItem>,
}

pub fn theme_status() -> ThemeStatus {
    let _ = ensure_themes();
    let themes = theme_list();
    let id = selected_id(&themes);
    let title = themes
        .iter()
        .find(|item| item.id == id)
        .map(|item| item.title.clone())
        .unwrap_or_else(|| id.clone());
    let path = theme_file(&id)
        .map(|path| path.display().to_string())
        .unwrap_or_default();
    ThemeStatus {
        id,
        title,
        path,
        resource_dir: crate::content::dir().display().to_string(),
        themes,
    }
}

pub fn theme_set(theme_id: String) -> Result<ThemeStatus, String> {
    let _ = ensure_themes();
    let id = theme_id.trim();
    if !theme_list().iter().any(|item| item.id == id) {
        return Err("没有这个主题。".into());
    }
    let Some(dir) = crate::content::armor_dir() else {
        return Err("找不到用户目录，主题没能保存。".into());
    };
    std::fs::create_dir_all(&dir).map_err(|err| format!("无法保存主题：{err}"))?;
    std::fs::write(dir.join("theme-id.txt"), id).map_err(|err| format!("无法保存主题：{err}"))?;
    Ok(theme_status())
}

const CHROME_CSS: &str = r#"

/* ============================================================
 * [壳强制·品牌区] 内嵌页左上 logo / 名字 / 徽标
 * 标题与 icon 改由壳顶栏显示，这里一律藏掉。
 * ============================================================ */
html [class*="_brand_"],
html [class*="_brandMark_"],
html [class*="_brandName_"],
html [class*="_brandBadge_"] {
  display: none !important;
}

/* ============================================================
 * [壳强制·新会话] 侧栏「新会话」按钮
 * 主题里即便给过样式，这里仍强制隐藏。
 * ============================================================ */
html [class*="_newSession_"] {
  display: none !important;
}
"#;

fn theme_css() -> String {
    let _ = ensure_themes();
    let id = selected_id(&theme_list());
    let base = if let Some(path) = theme_file(&id) {
        if let Ok(text) = std::fs::read_to_string(path) {
            if !text.trim().is_empty() {
                text
            } else {
                include_str!("../../themes/animal-crossing.css").to_string()
            }
        } else {
            include_str!("../../themes/animal-crossing.css").to_string()
        }
    } else {
        include_str!("../../themes/animal-crossing.css").to_string()
    };
    let mut out = base;
    out.push_str(CHROME_CSS);
    out
}

fn theme_list() -> Vec<ThemeItem> {
    let mut themes = Vec::new();
    let Ok(read) = std::fs::read_dir(themes_folder()) else {
        return themes;
    };
    for entry in read.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("css") {
            continue;
        }
        let Some(id) = path.file_stem().and_then(|name| name.to_str()) else {
            continue;
        };
        if !safe_id(id) {
            continue;
        }
        themes.push(ThemeItem {
            id: id.to_string(),
            title: title_of(&path, id),
        });
    }
    themes.sort_by_key(|item| (theme_rank(&item.id), item.title.clone()));
    themes
}

fn selected_id(themes: &[ThemeItem]) -> String {
    let saved = choice_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .unwrap_or_default();
    let id = saved.trim();
    if themes.iter().any(|item| item.id == id) {
        return id.to_string();
    }
    if themes.iter().any(|item| item.id == "animal-crossing") {
        return "animal-crossing".into();
    }
    themes
        .first()
        .map(|item| item.id.clone())
        .unwrap_or_else(|| "plain".into())
}

fn theme_file(id: &str) -> Option<PathBuf> {
    if !safe_id(id) {
        return None;
    }
    let path = themes_folder().join(format!("{id}.css"));
    path.is_file().then_some(path)
}

fn ensure_themes() -> Result<(), String> {
    crate::content::ensure()
}

fn title_of(path: &Path, id: &str) -> String {
    let Ok(text) = std::fs::read_to_string(path) else {
        return id.to_string();
    };
    let Some(start) = text.find("/*") else {
        return id.to_string();
    };
    let rest = &text[start + 2..];
    let Some(end) = rest.find("*/") else {
        return id.to_string();
    };
    let line = rest[..end].lines().next().unwrap_or("").trim();
    if line.is_empty() {
        id.to_string()
    } else {
        line.to_string()
    }
}

fn theme_rank(id: &str) -> usize {
    const ORDER: [&str; 8] = [
        "plain",
        "cream",
        "pastel",
        "cloud",
        "dopamine",
        "jade",
        "maillard",
        "animal-crossing",
    ];
    ORDER.iter().position(|item| *item == id).unwrap_or(100)
}

fn safe_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

fn themes_folder() -> PathBuf {
    crate::content::themes_dir()
}

fn choice_path() -> Option<PathBuf> {
    Some(crate::content::armor_dir()?.join("theme-id.txt"))
}

fn read_body(stream: &mut TcpStream, head: &str, mut body: Vec<u8>) -> std::io::Result<Vec<u8>> {
    if let Some(len) = content_length(head) {
        if len > BODY_CAP {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "html too large",
            ));
        }
        let need = len;
        let mut buf = body;
        read_exact(stream, &mut buf, need)?;
        buf.truncate(need);
        return Ok(buf);
    }
    let mut tmp = [0u8; 8192];
    loop {
        if body.len() >= BODY_CAP {
            break;
        }
        let n = stream.read(&mut tmp)?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&tmp[..n]);
    }
    Ok(body)
}

fn read_headers(stream: &mut TcpStream, buf: &mut Vec<u8>) -> std::io::Result<usize> {
    let mut tmp = [0u8; 4096];
    loop {
        if let Some(end) = header_end(buf) {
            return Ok(end);
        }
        if buf.len() > HEADER_CAP {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "headers too large",
            ));
        }
        let n = stream.read(&mut tmp)?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "headers",
            ));
        }
        buf.extend_from_slice(&tmp[..n]);
    }
}

fn read_exact(stream: &mut TcpStream, buf: &mut Vec<u8>, need: usize) -> std::io::Result<()> {
    let mut tmp = [0u8; 8192];
    while buf.len() < need {
        let want = (need - buf.len()).min(tmp.len());
        let n = stream.read(&mut tmp[..want])?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "body",
            ));
        }
        buf.extend_from_slice(&tmp[..n]);
    }
    Ok(())
}

fn header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|item| item == b"\r\n\r\n").map(|index| index + 4)
}

fn request_line(head: &str) -> Option<(&str, &str)> {
    let line = head.split("\r\n").next()?;
    let mut parts = line.split_whitespace();
    Some((parts.next()?, parts.next()?))
}

fn header<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    for line in head.split("\r\n") {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        if key.eq_ignore_ascii_case(name) {
            return Some(value.trim());
        }
    }
    None
}

fn content_length(head: &str) -> Option<usize> {
    header(head, "content-length")?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_lands_after_official_sheet() {
        let html = "<head><link rel=\"stylesheet\" href=\"/assets/app.css\"></head>";
        let out = inject_theme(html);
        let sheet = out.find("/assets/app.css").unwrap();
        let extra = out.find(THEME_PATH).unwrap();
        let close = out.find("</head>").unwrap();
        assert!(sheet < extra && extra < close);
    }

    #[test]
    fn proxy_injects_and_serves_theme() {
        let upstream = mock_upstream();
        let proxy = start_proxy(upstream, None).unwrap();
        let page = http_get(proxy.port, "/");
        assert!(page.contains("200"));
        let sheet = page.find("/assets/app.css").unwrap();
        let extra = page.find(THEME_PATH).unwrap();
        assert!(sheet < extra);
        let css = http_get(proxy.port, THEME_PATH);
        assert!(css.contains("text/css"));
        assert!(css.contains("pi-armor") || css.contains("--dsw-") || css.contains("颜色"));
        let posted = http_post(proxy.port, "/api/settings", "{\"appearance\":\"dark\"}");
        assert!(posted.contains("appearance"));
    }

    fn mock_upstream() -> SocketAddr {
        let listener = TcpListener::bind((HOST, 0)).unwrap();
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || {
            for _ in 0..4 {
                let Ok((mut stream, _)) = listener.accept() else { break };
                let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
                let mut buf = Vec::new();
                if read_headers(&mut stream, &mut buf).is_err() {
                    continue;
                }
                let end = header_end(&buf).unwrap();
                let head = String::from_utf8_lossy(&buf[..end]).into_owned();
                let len = content_length(&head).unwrap_or(0);
                let _ = read_exact(&mut stream, &mut buf, end + len);
                let body = &buf[end..end + len.min(buf.len().saturating_sub(end))];
                let path = request_line(&head).map(|(_, target)| target).unwrap_or("/");
                let (kind, payload) = if path.starts_with("/api/") {
                    ("application/json", body.to_vec())
                } else if path.contains(".css") {
                    ("text/css", b"body{color:#111}".to_vec())
                } else {
                    (
                        "text/html; charset=utf-8",
                        b"<!doctype html><head><link rel=\"stylesheet\" href=\"/assets/app.css\"></head><body></body>".to_vec(),
                    )
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    payload.len()
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.write_all(&payload);
            }
        });
        addr
    }

    fn http_get(port: u16, path: &str) -> String {
        exchange(port, &format!("GET {path} HTTP/1.1\r\nHost: {HOST}\r\nConnection: close\r\n\r\n"))
    }

    fn http_post(port: u16, path: &str, body: &str) -> String {
        exchange(
            port,
            &format!(
                "POST {path} HTTP/1.1\r\nHost: {HOST}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            ),
        )
    }

    fn exchange(port: u16, request: &str) -> String {
        let mut stream = TcpStream::connect(loopback(port));
        for _ in 0..20 {
            if stream.is_ok() {
                break;
            }
            thread::sleep(Duration::from_millis(50));
            stream = TcpStream::connect(loopback(port));
        }
        let mut stream = stream.unwrap();
        let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
        stream.write_all(request.as_bytes()).unwrap();
        let mut buf = Vec::new();
        let _ = stream.read_to_end(&mut buf);
        String::from_utf8_lossy(&buf).into_owned()
    }
}
