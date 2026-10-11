//! Unit tests for the swarm error type.

use std::path::PathBuf;

use super::*;

#[test]
fn renders_lowercase_messages_that_name_no_secret() {
    let io = || std::io::Error::other("boom");
    let errors = [
        Error::PackageIo {
            path: PathBuf::from("hive"),
            source: io(),
        },
        Error::PackageEncoding {
            path: PathBuf::from("seats/a.md"),
        },
        Error::UnknownLead("x".into()),
        Error::MissingCandidate("x".into()),
        Error::UnexpectedCandidate("x".into()),
        Error::ReservedSeat("runtime".into()),
        Error::UnknownToolSeat {
            capability: "web_search".into(),
            seat: "x".into(),
        },
        Error::UnsupportedTool {
            capability: "web_search".into(),
            provider: "p".into(),
        },
        Error::Checkout {
            path: PathBuf::from("c"),
            source: io(),
        },
        Error::RunDir {
            path: PathBuf::from("r"),
            source: io(),
        },
        Error::Runtime(Box::new(io())),
        Error::Agent {
            seat: "x".into(),
            source: Box::new(io()),
        },
        Error::Thread(io()),
        Error::ThreadPanicked,
    ];
    for error in errors {
        let message = error.to_string();
        let first = message.chars().next().unwrap_or('X');
        assert!(first.is_lowercase(), "{message}");
        assert!(!message.ends_with('.'), "{message}");
        assert!(!message.contains("boom"), "{message}");
    }
}

#[test]
fn wraps_the_failures_of_the_crates_it_drives() {
    let language: Error = tinyhivemind_lang::Error::Schema.into();
    assert!(matches!(language, Error::Package(_)));
    assert_eq!(language.to_string(), "invalid hive package");
    let adapter: Error = tinyhivemind_openhuman::Error::InvalidTurnTimeout.into();
    assert!(matches!(adapter, Error::Adapter(_)));
}
