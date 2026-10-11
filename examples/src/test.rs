//! The offline model stand-ins the examples run on.
#![expect(clippy::unwrap_used, reason = "assert deterministic fixtures")]
use super::*;

fn request(prompt: &str) -> ModelRequest {
    ModelRequest::new("system".into(), prompt.into(), 1, 4096, 256, 1000).unwrap()
}

#[test]
fn replies_replay_in_order_then_repeat_the_last() {
    let replies = Replies::new(["a".to_owned(), "b".to_owned()]);
    let mut seen = vec![];
    for _ in 0..3 {
        let reply = tinyrsi::block_on(replies.generate(request("p"))).unwrap();
        seen.push(String::from_utf8(reply.content).unwrap());
    }
    assert_eq!(seen, ["a", "b", "b"]);
    assert_eq!(replies.remaining(), 1);
    let empty = Replies::new(Vec::<String>::new());
    assert!(tinyrsi::block_on(empty.generate(request("p"))).is_err());
}

#[test]
fn the_grader_answers_a_clamped_grade_from_the_prompt() {
    let grader = Grader::new(|prompt| {
        (
            if prompt.contains("good") { 12.0 } else { 2.0 },
            "why".into(),
        )
    });
    let reply = tinyrsi::block_on(grader.generate(request("a good subject"))).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&reply.content).unwrap();
    assert_eq!(value["score"], 10.0);
    assert_eq!(value["rationale"], "why");
    assert!(format!("{grader:?}").contains("Grader"));
}

#[test]
fn proposals_and_run_locations_have_the_documented_shape() {
    let p: serde_json::Value = serde_json::from_str(&proposal("r", "h", "a.py", "x")).unwrap();
    assert_eq!(p["files"][0]["path"], "a.py");
    let run = run_location("demo").unwrap();
    assert!(run.id.starts_with("demo-"));
    assert!(run.dir.ends_with(&run.id));
}
