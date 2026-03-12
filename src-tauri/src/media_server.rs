use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::OnceLock;
use std::{fs::File, io::Seek, io::SeekFrom};

use tauri::http::{header, StatusCode};

static BASE_URL: OnceLock<String> = OnceLock::new();
static STARTED: OnceLock<()> = OnceLock::new();

fn guess_mime(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "webm" => "video/webm",
        "mp4" => "video/mp4",
        "mkv" => "video/x-matroska",
        "mov" => "video/quicktime",
        "avi" => "video/x-msvideo",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => "application/octet-stream",
    }
}

pub fn get_base_url() -> Option<String> {
    BASE_URL.get().cloned()
}

pub fn start() {
    if STARTED.set(()).is_err() {
        return;
    }

    std::thread::spawn(|| {
        let listener = match TcpListener::bind("127.0.0.1:0") {
            Ok(l) => l,
            Err(e) => {
                log::warn!("媒体服务：绑定端口失败：{}", e);
                return;
            }
        };

        let addr = match listener.local_addr() {
            Ok(a) => a,
            Err(e) => {
                log::warn!("媒体服务：获取本地地址失败：{}", e);
                return;
            }
        };

        let base = format!("http://127.0.0.1:{}", addr.port());
        let _ = BASE_URL.set(base.clone());
        log::info!("媒体服务：已启动 base_url={}", base);

        for stream in listener.incoming() {
            let mut stream = match stream {
                Ok(s) => s,
                Err(_) => continue,
            };

            let mut buf = [0u8; 8192];
            let n = match stream.read(&mut buf) {
                Ok(n) => n,
                Err(_) => continue,
            };
            if n == 0 {
                continue;
            }

            let req = String::from_utf8_lossy(&buf[..n]);
            let mut lines = req.lines();
            let first = match lines.next() {
                Some(l) => l,
                None => continue,
            };
            let mut parts = first.split_whitespace();
            let method = parts.next().unwrap_or("GET");
            let target = parts.next().unwrap_or("/");

            let mut range_header: Option<String> = None;
            for l in lines {
                let lower = l.to_ascii_lowercase();
                if lower.starts_with("range:") {
                    range_header = Some(l.to_string());
                    break;
                }
            }

            if method == "OPTIONS" {
                let resp = format!(
                    "HTTP/1.1 {}\r\n{}: *\r\n{}: *\r\n{}: GET,HEAD,OPTIONS\r\nContent-Length: 0\r\n\r\n",
                    StatusCode::NO_CONTENT.as_u16(),
                    header::ACCESS_CONTROL_ALLOW_ORIGIN,
                    header::ACCESS_CONTROL_ALLOW_HEADERS,
                    header::ACCESS_CONTROL_ALLOW_METHODS,
                );
                let _ = stream.write_all(resp.as_bytes());
                continue;
            }

            if !target.starts_with("/file") {
                let resp = "HTTP/1.1 404\r\nContent-Length: 0\r\n\r\n";
                let _ = stream.write_all(resp.as_bytes());
                continue;
            }

            let path_param = target.splitn(2, '?').nth(1).unwrap_or("");
            let mut file_path_raw: Option<String> = None;
            for kv in path_param.split('&') {
                let mut kv_it = kv.splitn(2, '=');
                let k = kv_it.next().unwrap_or("");
                let v = kv_it.next().unwrap_or("");
                if k == "path" {
                    file_path_raw = Some(v.to_string());
                    break;
                }
            }

            let Some(raw) = file_path_raw else {
                let resp = "HTTP/1.1 400\r\nContent-Length: 0\r\n\r\n";
                let _ = stream.write_all(resp.as_bytes());
                continue;
            };

            let decoded = urlencoding::decode(&raw)
                .map(|s| s.into_owned())
                .unwrap_or(raw);

            let req_path = Path::new(&decoded);
            let data_dir = crate::library::get_data_dir();
            if !req_path.starts_with(&data_dir) {
                let resp = "HTTP/1.1 403\r\nContent-Length: 0\r\n\r\n";
                let _ = stream.write_all(resp.as_bytes());
                continue;
            }

            let meta = match std::fs::metadata(req_path) {
                Ok(m) => m,
                Err(_) => {
                    let resp = "HTTP/1.1 404\r\nContent-Length: 0\r\n\r\n";
                    let _ = stream.write_all(resp.as_bytes());
                    continue;
                }
            };

            let len = meta.len();
            let mime = guess_mime(req_path);

            if method == "HEAD" {
                let resp = format!(
                    "HTTP/1.1 200\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\n{}: *\r\n\r\n",
                    mime,
                    len,
                    header::ACCESS_CONTROL_ALLOW_ORIGIN,
                );
                let _ = stream.write_all(resp.as_bytes());
                continue;
            }

            // Range support: WebKit/GStreamer often relies on byte-range requests for seek and loop.
            // We only implement single-range requests: Range: bytes=start-end
            if let Some(r) = range_header.as_deref() {
                let r = r.trim();
                let mut start: Option<u64> = None;
                let mut end: Option<u64> = None;

                // Example: "Range: bytes=123-" or "Range: bytes=123-456"
                if let Some(eq) = r.find('=') {
                    let unit_and_value = &r[eq + 1..];
                    if unit_and_value.starts_with("bytes") {
                        if let Some(dash) = unit_and_value.find('-') {
                            let (a, b) = unit_and_value[5..].split_at(dash.saturating_sub(5));
                            let a = a.trim();
                            let b = unit_and_value[5..].splitn(2, '-').nth(1).unwrap_or("").trim();
                            if !a.is_empty() {
                                start = a.parse::<u64>().ok();
                            }
                            if !b.is_empty() {
                                end = b.parse::<u64>().ok();
                            }
                        }
                    } else if unit_and_value.starts_with("bytes=") {
                        let v = &unit_and_value[6..];
                        let mut it = v.splitn(2, '-');
                        let a = it.next().unwrap_or("").trim();
                        let b = it.next().unwrap_or("").trim();
                        if !a.is_empty() {
                            start = a.parse::<u64>().ok();
                        }
                        if !b.is_empty() {
                            end = b.parse::<u64>().ok();
                        }
                    }
                }

                let start = start.unwrap_or(0);
                let end = end.unwrap_or_else(|| len.saturating_sub(1));
                if start >= len || end < start {
                    let resp = format!(
                        "HTTP/1.1 416\r\nContent-Range: bytes */{}\r\nContent-Length: 0\r\nAccept-Ranges: bytes\r\n{}: *\r\n\r\n",
                        len,
                        header::ACCESS_CONTROL_ALLOW_ORIGIN,
                    );
                    let _ = stream.write_all(resp.as_bytes());
                    continue;
                }

                let end = end.min(len.saturating_sub(1));
                let to_read = end - start + 1;

                let mut f = match File::open(req_path) {
                    Ok(f) => f,
                    Err(_) => {
                        let resp = "HTTP/1.1 500\r\nContent-Length: 0\r\n\r\n";
                        let _ = stream.write_all(resp.as_bytes());
                        continue;
                    }
                };
                if f.seek(SeekFrom::Start(start)).is_err() {
                    let resp = "HTTP/1.1 500\r\nContent-Length: 0\r\n\r\n";
                    let _ = stream.write_all(resp.as_bytes());
                    continue;
                }

                let header_txt = format!(
                    "HTTP/1.1 206\r\nContent-Type: {}\r\nContent-Length: {}\r\nContent-Range: bytes {}-{}/{}\r\nAccept-Ranges: bytes\r\n{}: *\r\n\r\n",
                    mime,
                    to_read,
                    start,
                    end,
                    len,
                    header::ACCESS_CONTROL_ALLOW_ORIGIN,
                );
                if stream.write_all(header_txt.as_bytes()).is_ok() {
                    let mut remaining = to_read;
                    let mut chunk = [0u8; 64 * 1024];
                    while remaining > 0 {
                        let want = (remaining as usize).min(chunk.len());
                        match f.read(&mut chunk[..want]) {
                            Ok(0) => break,
                            Ok(n) => {
                                remaining = remaining.saturating_sub(n as u64);
                                let _ = stream.write_all(&chunk[..n]);
                            }
                            Err(_) => break,
                        }
                    }
                }
                continue;
            }

            // No range: return full body.
            match std::fs::read(req_path) {
                Ok(bytes) => {
                    let header_txt = format!(
                        "HTTP/1.1 200\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\n{}: *\r\n\r\n",
                        mime,
                        bytes.len(),
                        header::ACCESS_CONTROL_ALLOW_ORIGIN,
                    );
                    if stream.write_all(header_txt.as_bytes()).is_ok() {
                        let _ = stream.write_all(&bytes);
                    }
                }
                Err(_) => {
                    let resp = "HTTP/1.1 500\r\nContent-Length: 0\r\n\r\n";
                    let _ = stream.write_all(resp.as_bytes());
                }
            }
        }
    });
}
