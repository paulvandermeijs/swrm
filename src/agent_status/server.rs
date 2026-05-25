use anyhow::{Context, Result};
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

/// One hook delivery from a tab's settings file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookEvent {
    pub tab_id: String,
    pub event: String,
}

/// Bind a TCP listener on `127.0.0.1:0` (kernel-assigned port), spawn a
/// background thread that accepts hook POSTs, and return the bound port
/// plus a receiver of parsed `HookEvent`s. The thread runs for the
/// lifetime of the process — there is no explicit shutdown signal, and
/// the OS reclaims the socket on exit.
pub fn start_server() -> Result<(u16, UnboundedReceiver<HookEvent>)> {
    let listener = TcpListener::bind("127.0.0.1:0").context("bind agent-status server")?;
    let port = listener
        .local_addr()
        .context("agent-status server local_addr")?
        .port();
    let (tx, rx) = unbounded::<HookEvent>();
    std::thread::Builder::new()
        .name("swrm-agent-status-server".into())
        .spawn(move || run_accept_loop(listener, tx))
        .context("spawn agent-status server thread")?;
    Ok((port, rx))
}

/// Extract `(tab_id, event)` from a request path like `/event/<tab_id>/<event>`.
/// Returns `None` for any other shape (wrong prefix, missing segment, extra
/// segments). Query strings (`?…`) are stripped before parsing.
pub fn parse_event_path(path: &str) -> Option<(&str, &str)> {
    let path = path.split('?').next().unwrap_or(path);
    let rest = path.strip_prefix("/event/")?;
    // Exactly two non-empty segments.
    let mut parts = rest.split('/');
    let tab_id = parts.next()?;
    let event = parts.next()?;
    if tab_id.is_empty() || event.is_empty() {
        return None;
    }
    if parts.next().is_some() {
        return None;
    }
    Some((tab_id, event))
}

fn run_accept_loop(listener: TcpListener, tx: UnboundedSender<HookEvent>) {
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        // Hook handlers run synchronously and complete in microseconds; no
        // worker pool needed. If a future agent fires many concurrent hooks
        // we can revisit.
        if let Err(err) = handle_one(stream, &tx) {
            tracing::debug!(?err, "hook request handling failed");
        }
    }
}

fn handle_one(mut stream: TcpStream, tx: &UnboundedSender<HookEvent>) -> Result<()> {
    // We only need the request line. Cap at 1 KiB to bound memory.
    let mut buf = [0u8; 1024];
    let n = stream.read(&mut buf).context("read hook request")?;
    let req = &buf[..n];
    let line_end = req
        .iter()
        .position(|&b| b == b'\r' || b == b'\n')
        .unwrap_or(req.len());
    let line = std::str::from_utf8(&req[..line_end]).unwrap_or("");
    // "POST /event/<tab>/<event> HTTP/1.1"
    let mut parts = line.split(' ');
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("");
    if method == "POST" {
        if let Some((tab_id, event)) = parse_event_path(path) {
            let _ = tx.unbounded_send(HookEvent {
                tab_id: tab_id.to_string(),
                event: event.to_string(),
            });
        }
    }
    // Reply 204 unconditionally — curl exits 0 whether we recognised the
    // request or not, which is fine because hook scripts ignore the response.
    let _ = stream.write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n");
    Ok(())
}
