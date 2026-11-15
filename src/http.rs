//! A micro HTTP/1.1 server built straight on `std::net`.
//!
//! It serves the embedded UI and the JSON control API. One thread per
//! connection, `Connection: close`, no keep-alive — perfect for a local
//! single-user control surface.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;

pub struct Request {
    pub method: String,
    pub path: String,
    pub query: Vec<(String, String)>,
    pub body: String,
}

impl Request {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.query
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Query value or empty string.
    pub fn q(&self, key: &str) -> String {
        self.get(key).unwrap_or("").to_string()
    }
}

pub struct Response {
    pub status: u16,
    pub content_type: String,
    pub body: Vec<u8>,
}

impl Response {
    pub fn new(status: u16, content_type: &str, body: Vec<u8>) -> Response {
        Response {
            status,
            content_type: content_type.to_string(),
            body,
        }
    }

    pub fn html(body: &str) -> Response {
        Response::new(200, "text/html; charset=utf-8", body.as_bytes().to_vec())
    }

    pub fn js(body: &str) -> Response {
        Response::new(
            200,
            "application/javascript; charset=utf-8",
            body.as_bytes().to_vec(),
        )
    }

    pub fn css(body: &str) -> Response {
        Response::new(200, "text/css; charset=utf-8", body.as_bytes().to_vec())
    }

    pub fn json(body: String) -> Response {
        Response::new(200, "application/json; charset=utf-8", body.into_bytes())
    }

    pub fn text(status: u16, body: &str) -> Response {
        Response::new(
            status,
            "text/plain; charset=utf-8",
            body.as_bytes().to_vec(),
        )
    }

    pub fn not_found() -> Response {
        Response::text(404, "not found")
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        500 => "Internal Server Error",
        _ => "OK",
    }
}

fn hexval(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

#[allow(dead_code)]
pub fn decode_percent(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hi = hexval(bytes[i + 1]);
                let lo = hexval(bytes[i + 2]);
                match (hi, lo) {
                    (Some(a), Some(b)) => {
                        out.push(a * 16 + b);
                        i += 3;
                    }
                    _ => {
                        out.push(bytes[i]);
                        i += 1;
                    }
                }
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

fn parse_query(raw: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if raw.is_empty() {
        return out;
    }
    for pair in raw.split('&') {
        if pair.is_empty() {
            continue;
        }
        let mut it = pair.splitn(2, '=');
        let k = it.next().unwrap_or("");
        let v = it.next().unwrap_or("");
        out.push((decode_percent(k), decode_percent(v)));
    }
    out
}

pub type Handler = Arc<dyn Fn(Request) -> Response + Send + Sync>;

/// Bind and serve forever. `bind` looks like `127.0.0.1:8787`.
pub fn serve(bind: &str, handler: Handler) -> std::io::Result<()> {
    let listener = TcpListener::bind(bind)?;
    for incoming in listener.incoming() {
        match incoming {
            Ok(stream) => {
                let h = handler.clone();
                thread::spawn(move || {
                    let _ = handle(stream, h);
                });
            }
            Err(_) => continue,
        }
    }
    Ok(())
}

fn handle(stream: TcpStream, handler: Handler) -> std::io::Result<()> {
    let mut stream = stream;
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(10)));
    let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(10)));

    let reader_stream = match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return Ok(()),
    };
    let mut reader = BufReader::new(reader_stream);

    let mut line = String::new();
    if reader.read_line(&mut line).unwrap_or(0) == 0 {
        return Ok(());
    }
    let mut parts = line.trim().split_whitespace();
    let method = parts.next().unwrap_or("GET").to_uppercase();
    let target = parts.next().unwrap_or("/").to_string();

    let mut content_length: usize = 0;
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h).unwrap_or(0) == 0 {
            break;
        }
        let trimmed = h.trim_end();
        if trimmed.is_empty() {
            break;
        }
        let lower = trimmed.to_lowercase();
        if let Some(rest) = lower.strip_prefix("content-length:") {
            content_length = rest.trim().parse::<usize>().unwrap_or(0);
        }
    }

    let mut body = String::new();
    if content_length > 0 {
        let take = std::cmp::min(content_length, 512 * 1024);
        let mut buf = vec![0u8; take];
        if reader.read_exact(&mut buf).is_ok() {
            body = String::from_utf8_lossy(&buf).to_string();
        }
    }

    let mut path = target.clone();
    let mut raw_query = String::new();
    if let Some(pos) = target.find('?') {
        path = target[..pos].to_string();
        raw_query = target[pos + 1..].to_string();
    }

    let req = Request {
        method,
        path,
        query: parse_query(&raw_query),
        body,
    };

    let res = handler(req);

    let mut head = String::new();
    head.push_str(&format!("HTTP/1.1 {} {}\r\n", res.status, reason(res.status)));
    head.push_str(&format!("Content-Type: {}\r\n", res.content_type));
    head.push_str(&format!("Content-Length: {}\r\n", res.body.len()));
    head.push_str("Cache-Control: no-store\r\n");
    head.push_str("Access-Control-Allow-Origin: *\r\n");
    head.push_str("Access-Control-Allow-Headers: content-type\r\n");
    head.push_str("Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n");
    head.push_str("Connection: close\r\n\r\n");

    stream.write_all(head.as_bytes())?;
    stream.write_all(&res.body)?;
    let _ = stream.flush();
    Ok(())
}
