//! A local, read-only web UI over the run store.
//!
//! A deliberately small HTTP/1.1 server on the standard library: one thread
//! per connection, `GET` only, loopback by default. It serves one embedded
//! page and a JSON and Server-Sent Events API over the store:
//!
//! | Path | Body |
//! | --- | --- |
//! | `/` | the UI page |
//! | `/api/runs` | every run's `run.json`, newest first |
//! | `/api/runs/<loop>/<run>/events` | `events.jsonl` replayed then tailed, as SSE |
//! | `/api/runs/<loop>/<run>/files/<path>` | `run.json`, `ledger.json`, an epoch's `brief.md`, or a candidate's `change.diff` |
//!
//! Every path segment is validated as a plain name before it touches the
//! filesystem, so a request cannot leave the store.

use std::{
    fs,
    io::{BufRead, BufReader, Read, Seek, SeekFrom, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use crate::{Error, Result, store::Store};

/// The UI page.
const PAGE: &str = include_str!("ui.html");

/// How often a tailed trace is polled for new lines.
const POLL: Duration = Duration::from_millis(400);

/// How often an idle event stream sends a heartbeat comment.
const HEARTBEAT: Duration = Duration::from_secs(15);

/// A running server.
#[derive(Debug)]
pub struct Server {
    addr: SocketAddr,
}

impl Server {
    /// The address it listens on.
    #[must_use]
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// The UI's URL.
    #[must_use]
    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }
}

/// Start serving `store` on `addr` from a background thread.
///
/// # Errors
/// Returns [`Error::Io`] when the address cannot be bound.
pub fn start(store: Store, addr: SocketAddr) -> Result<Server> {
    let listener =
        TcpListener::bind(addr).map_err(|e| Error::io("listen on", addr.to_string(), e))?;
    let addr = listener
        .local_addr()
        .map_err(|e| Error::io("listen on", addr.to_string(), e))?;
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let store = store.clone();
            thread::spawn(move || {
                let _ = handle(&store, stream);
            });
        }
    });
    Ok(Server { addr })
}

fn handle(store: &Store, mut stream: TcpStream) -> std::io::Result<()> {
    let root = store.runs_root();
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    // Drain the headers; nothing here needs them.
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 || line.trim().is_empty() {
            break;
        }
    }
    let mut parts = request_line.split_whitespace();
    let (method, target) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
    if method != "GET" {
        return respond(&mut stream, 405, "text/plain", b"method not allowed");
    }
    let path = target.split('?').next().unwrap_or("");
    match route(path) {
        Route::Page => respond(
            &mut stream,
            200,
            "text/html; charset=utf-8",
            PAGE.as_bytes(),
        ),
        Route::Runs => {
            let runs = store.list_runs(None).unwrap_or_default();
            let body = serde_json::to_vec(&runs).unwrap_or_else(|_| b"[]".to_vec());
            respond(&mut stream, 200, "application/json", &body)
        }
        Route::Events(run) => stream_events(&mut stream, &root.join(run).join("events.jsonl")),
        Route::File(run, file) => match fs::read(root.join(run).join(&file)) {
            Ok(body) => respond(&mut stream, 200, content_type(&file), &body),
            Err(_) => respond(&mut stream, 404, "text/plain", b"not found"),
        },
        Route::NotFound => respond(&mut stream, 404, "text/plain", b"not found"),
    }
}

/// What a request path asks for.
#[derive(Debug, PartialEq, Eq)]
enum Route {
    Page,
    Runs,
    Events(PathBuf),
    File(PathBuf, PathBuf),
    NotFound,
}

fn route(path: &str) -> Route {
    let segments: Vec<&str> = path.trim_matches('/').split('/').collect();
    match segments.as_slice() {
        [""] => Route::Page,
        ["api", "runs"] => Route::Runs,
        ["api", "runs", l, r, "events"] if plain(l) && plain(r) => {
            Route::Events(Path::new(l).join(r))
        }
        ["api", "runs", l, r, "files", rest @ ..] if plain(l) && plain(r) => allowed(rest)
            .map_or(Route::NotFound, |file| {
                Route::File(Path::new(l).join(r), file)
            }),
        _ => Route::NotFound,
    }
}

/// A single safe path segment.
fn plain(segment: &str) -> bool {
    !segment.is_empty()
        && segment != "."
        && segment != ".."
        && segment
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// The files a run exposes.
fn allowed(rest: &[&str]) -> Option<PathBuf> {
    let epoch = |s: &str| {
        s.strip_prefix("epoch-")
            .is_some_and(|n| n.parse::<u32>().is_ok())
    };
    match rest {
        [name @ ("run.json" | "ledger.json")] => Some(PathBuf::from(name)),
        [e, "brief.md"] if epoch(e) => Some(Path::new(e).join("brief.md")),
        [e, "steps", c, "change.diff"] if epoch(e) && plain(c) => {
            Some(Path::new(e).join("steps").join(c).join("change.diff"))
        }
        _ => None,
    }
}

fn content_type(file: &Path) -> &'static str {
    match file.extension().and_then(|e| e.to_str()) {
        Some("json") => "application/json",
        Some("md") => "text/markdown; charset=utf-8",
        _ => "text/plain; charset=utf-8",
    }
}

fn respond(stream: &mut TcpStream, status: u16, kind: &str, body: &[u8]) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        404 => "Not Found",
        _ => "Method Not Allowed",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)?;
    stream.flush()
}

/// Replay `events.jsonl`, then send new complete lines as they appear, until
/// the client goes away. Each batch is one SSE message with a `data:` line
/// per trace line; `event: ready` follows the replay.
fn stream_events(stream: &mut TcpStream, file: &Path) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\nretry: 2000\n\n"
    )?;
    let mut offset = 0_u64;
    let mut partial = String::new();
    let mut ready = false;
    let mut last_sent = Instant::now();
    loop {
        let lines = read_new_lines(file, &mut offset, &mut partial);
        if !lines.is_empty() {
            let message: String = lines.iter().map(|l| format!("data: {l}\n")).collect();
            write!(stream, "{message}\n")?;
            last_sent = Instant::now();
        }
        if !ready {
            write!(stream, "event: ready\ndata: {{}}\n\n")?;
            ready = true;
        }
        if last_sent.elapsed() >= HEARTBEAT {
            write!(stream, ": ping\n\n")?;
            last_sent = Instant::now();
        }
        stream.flush()?;
        thread::sleep(POLL);
    }
}

/// Complete lines appended to `file` since `offset`.
fn read_new_lines(file: &Path, offset: &mut u64, partial: &mut String) -> Vec<String> {
    let Ok(mut handle) = fs::File::open(file) else {
        return Vec::new();
    };
    let len = handle.metadata().map_or(0, |m| m.len());
    if len < *offset {
        *offset = 0;
        partial.clear();
    }
    if handle.seek(SeekFrom::Start(*offset)).is_err() {
        return Vec::new();
    }
    let mut chunk = String::new();
    if handle.read_to_string(&mut chunk).is_err() {
        return Vec::new();
    }
    *offset += chunk.len() as u64;
    partial.push_str(&chunk);
    let mut lines = Vec::new();
    while let Some(end) = partial.find('\n') {
        let line: String = partial.drain(..=end).collect();
        let line = line.trim();
        if !line.is_empty() {
            lines.push(line.to_owned());
        }
    }
    lines
}

#[cfg(test)]
mod test;
