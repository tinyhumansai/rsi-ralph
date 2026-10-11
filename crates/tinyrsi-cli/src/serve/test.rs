//! Routing, path safety and the live HTTP surface.
#![expect(clippy::unwrap_used, reason = "assert deterministic fixtures")]
use super::*;
use crate::store::{RunRecord, RunStatus, write_json};
use std::collections::BTreeMap;

fn get(server: &Server, path: &str, method: &str) -> String {
    let mut stream = TcpStream::connect(server.addr()).unwrap();
    write!(stream, "{method} {path} HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
    let mut out = String::new();
    stream.read_to_string(&mut out).unwrap();
    out
}

fn served() -> (tempfile::TempDir, Server) {
    let home = tempfile::tempdir().unwrap();
    let store = Store::new(home.path());
    let run = store
        .create_run("demo", Some("r1"), &BTreeMap::new())
        .unwrap();
    write_json(
        &run.record_path(),
        &RunRecord {
            id: "r1".into(),
            r#loop: "demo".into(),
            status: RunStatus::Finished,
            pid: 1,
            started_at: 5,
            finished_at: Some(6),
            stop_reason: None,
            incumbent: "abc".into(),
            incumbent_score: Some(1.5),
            epochs: 1,
            spend_usd: 0.0,
            error: None,
        },
    )
    .unwrap();
    fs::write(run.events_path(), "{\"seq\":0}\n{\"seq\":1}\n").unwrap();
    fs::write(run.epoch_dir(1).unwrap().join("brief.md"), "# brief").unwrap();
    let server = start(store, "127.0.0.1:0".parse().unwrap()).unwrap();
    (home, server)
}

#[test]
fn routes_only_safe_paths() {
    assert_eq!(route("/"), Route::Page);
    assert_eq!(route("/api/runs"), Route::Runs);
    assert_eq!(
        route("/api/runs/l/r/events"),
        Route::Events(PathBuf::from("l/r"))
    );
    assert_eq!(
        route("/api/runs/l/r/files/ledger.json"),
        Route::File(PathBuf::from("l/r"), PathBuf::from("ledger.json"))
    );
    assert_eq!(
        route("/api/runs/l/r/files/epoch-2/steps/e2-a/change.diff"),
        Route::File(
            PathBuf::from("l/r"),
            PathBuf::from("epoch-2/steps/e2-a/change.diff")
        )
    );
    for bad in [
        "/api/runs/../r/events",
        "/api/runs/l/r/files/../../etc/passwd",
        "/api/runs/l/r/files/loop/rsi.json",
        "/api/runs/l/r/files/epoch-x/brief.md",
        "/api/runs/l/%2e%2e/events",
        "/other",
    ] {
        assert_eq!(route(bad), Route::NotFound, "{bad}");
    }
}

#[test]
fn serves_the_page_runs_and_files() {
    let (_home, server) = served();
    assert!(server.url().starts_with("http://127.0.0.1:"));
    let page = get(&server, "/", "GET");
    assert!(page.starts_with("HTTP/1.1 200") && page.contains("<title>tinyrsi runs</title>"));
    let runs = get(&server, "/api/runs", "GET");
    assert!(runs.contains("\"incumbent_score\":1.5"));
    let record = get(&server, "/api/runs/demo/r1/files/run.json", "GET");
    assert!(record.contains("application/json") && record.contains("\"abc\""));
    let brief = get(&server, "/api/runs/demo/r1/files/epoch-1/brief.md", "GET");
    assert!(brief.contains("text/markdown") && brief.ends_with("# brief"));
    assert!(get(&server, "/api/runs/demo/r1/files/ledger.json", "GET").starts_with("HTTP/1.1 404"));
    assert!(get(&server, "/nope", "GET").starts_with("HTTP/1.1 404"));
    assert!(get(&server, "/", "POST").starts_with("HTTP/1.1 405"));
}

#[test]
fn streams_events_then_marks_ready() {
    let (_home, server) = served();
    let mut stream = TcpStream::connect(server.addr()).unwrap();
    write!(stream, "GET /api/runs/demo/r1/events HTTP/1.1\r\n\r\n").unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut seen = String::new();
    let mut buf = [0_u8; 4096];
    while !seen.contains("event: ready") {
        let n = stream.read(&mut buf).unwrap();
        assert!(n > 0, "stream closed early: {seen}");
        seen.push_str(&String::from_utf8_lossy(&buf[..n]));
    }
    assert!(seen.contains("text/event-stream"));
    assert!(seen.contains("data: {\"seq\":0}\ndata: {\"seq\":1}\n\n"));
}

#[test]
fn tails_only_complete_new_lines_and_restarts_after_truncation() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("events.jsonl");
    let (mut offset, mut partial) = (0, String::new());
    assert!(read_new_lines(&file, &mut offset, &mut partial).is_empty());
    fs::write(&file, "a\nb").unwrap();
    assert_eq!(read_new_lines(&file, &mut offset, &mut partial), vec!["a"]);
    fs::write(&file, "a\nbc\n").unwrap();
    assert_eq!(read_new_lines(&file, &mut offset, &mut partial), vec!["bc"]);
    fs::write(&file, "z\n").unwrap();
    assert_eq!(read_new_lines(&file, &mut offset, &mut partial), vec!["z"]);
}

#[test]
fn a_taken_port_is_an_io_error() {
    let (_home, server) = served();
    let home = tempfile::tempdir().unwrap();
    assert!(matches!(
        start(Store::new(home.path()), server.addr()),
        Err(Error::Io { .. })
    ));
}
