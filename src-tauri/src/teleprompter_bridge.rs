//! Local-only bridge for feeding generated answers into eyeread.in.
//!
//! Listens on 127.0.0.1 only. This intentionally does not expose the bridge
//! to the LAN. NexQ can POST a script here without touching EyeRead's existing
//! storage or speech/matching pipeline.

use serde::Deserialize;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    thread,
};
use tauri::{AppHandle, Emitter};

const HOST: &str = "127.0.0.1:17842";

#[derive(Debug, Deserialize)]
struct PushRequest {
    text: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    id: Option<String>,
}

fn response(stream: &mut TcpStream, status: &str, body: &str) {
    let payload = body.as_bytes();
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: Content-Type\r\nAccess-Control-Allow-Methods: POST, OPTIONS\r\n\r\n",
        payload.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(payload);
}

fn handle(mut stream: TcpStream, app: &AppHandle) {
    let mut buf = vec![0u8; 256 * 1024];
    let Ok(n) = stream.read(&mut buf) else { return };
    let request = String::from_utf8_lossy(&buf[..n]);

    let Some((head, body)) = request.split_once("\r\n\r\n") else {
        response(&mut stream, "400 Bad Request", r#"{"ok":false,"error":"malformed_request"}"#);
        return;
    };

    let mut lines = head.lines();
    let request_line = lines.next().unwrap_or_default();
    if request_line.starts_with("OPTIONS ") {
        response(&mut stream, "204 No Content", "");
        return;
    }

    if !request_line.starts_with("POST /v1/teleprompter/script ") {
        response(&mut stream, "404 Not Found", r#"{"ok":false,"error":"not_found"}"#);
        return;
    }

    let content_length = lines
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("content-length") {
                value.trim().parse::<usize>().ok()
            } else {
                None
            }
        })
        .unwrap_or(0);

    // This bridge is intentionally small. Reject oversized bodies rather than
    // allocating unbounded memory or accepting partial JSON.
    if content_length == 0 || content_length > 200_000 || body.as_bytes().len() < content_length {
        response(&mut stream, "400 Bad Request", r#"{"ok":false,"error":"invalid_body"}"#);
        return;
    }

    let body = &body.as_bytes()[..content_length];
    let Ok(req) = serde_json::from_slice::<PushRequest>(body) else {
        response(&mut stream, "400 Bad Request", r#"{"ok":false,"error":"invalid_json"}"#);
        return;
    };

    let text = req.text.trim();
    if text.is_empty() {
        response(&mut stream, "422 Unprocessable Entity", r#"{"ok":false,"error":"text_required"}"#);
        return;
    }

    let script = serde_json::json!({
        "id": req.id.unwrap_or_else(|| format!("nexq-{}", chrono_like_timestamp())),
        "title": req.title.unwrap_or_else(|| "NexQ Answer".to_string()),
        "text": text,
        "tag": "ready",
        "pinned": false,
        "updatedAt": 0,
        "language": req.language.unwrap_or_else(|| "en-US".to_string())
    });

    if app.emit_to("overlay", "overlay:load", serde_json::json!({ "script": script })).is_err() {
        response(&mut stream, "503 Service Unavailable", r#"{"ok":false,"error":"overlay_unavailable"}"#);
        return;
    }

    response(&mut stream, "200 OK", r#"{"ok":true}"#);
}

// Dependency-free monotonic-ish identifier. It is only used as a temporary
// script id; persistence is not part of the bridge contract.
fn chrono_like_timestamp() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

pub fn start(app: AppHandle) {
    thread::spawn(move || {
        let Ok(listener) = TcpListener::bind(HOST) else {
            eprintln!("[teleprompter-bridge] unable to bind {HOST}");
            return;
        };
        eprintln!("[teleprompter-bridge] listening on {HOST}");

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => handle(stream, &app),
                Err(err) => eprintln!("[teleprompter-bridge] connection error: {err}"),
            }
        }
    });
}
