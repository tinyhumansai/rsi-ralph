//! Folder and git subjects: checkouts, changes, diffs and promotion.
#![expect(clippy::unwrap_used, reason = "assert deterministic fixtures")]
use super::*;

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

#[test]
fn a_folder_subject_copies_compares_and_promotes() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("loop/subject");
    write(&source.join("a.py"), "a = 1\n");
    write(&source.join("pkg/b.py"), "b = 1\n");
    write(&source.join("__pycache__/a.pyc"), "junk");
    let spec = SubjectSpec {
        kind: SubjectKind::Dir,
        path: "subject".into(),
        branch: None,
        editable: vec!["a.py".into()],
        frozen: vec![],
        require_test_change: false,
    };
    let subject = Subject::from_spec(&spec, &root.path().join("loop"));
    let run = root.path().join("run");
    let incumbent = subject.incumbent(&run).unwrap();
    assert!(!incumbent.path.join("__pycache__").exists());
    let before = subject.revision(&incumbent).unwrap();
    assert_eq!(before.len(), 12);

    let cand = subject
        .candidate(&incumbent, "c1", "unused", &run.join("epoch-1/c1"))
        .unwrap();
    assert_eq!(
        subject.changes(&incumbent, &cand).unwrap(),
        Vec::<String>::new()
    );
    write(&cand.path.join("a.py"), "a = 2\n");
    write(&cand.path.join("new.py"), "n\n");
    fs::remove_file(cand.path.join("pkg/b.py")).unwrap();
    assert_eq!(
        subject.changes(&incumbent, &cand).unwrap(),
        vec!["a.py", "new.py", "pkg/b.py"]
    );
    assert!(subject.diff(&incumbent, &cand).unwrap().contains("+a = 2"));

    subject.promote(&incumbent, &cand).unwrap();
    assert_eq!(
        fs::read_to_string(incumbent.path.join("a.py")).unwrap(),
        "a = 2\n"
    );
    assert_ne!(subject.revision(&incumbent).unwrap(), before);
    assert_eq!(fs::read_to_string(source.join("a.py")).unwrap(), "a = 1\n");
}

#[test]
fn a_missing_folder_subject_is_an_io_error() {
    let root = tempfile::tempdir().unwrap();
    let subject = Subject::Dir {
        source: root.path().join("none"),
    };
    assert!(matches!(
        subject.incumbent(root.path()),
        Err(Error::Io { .. })
    ));
}

fn repo(root: &Path) -> PathBuf {
    let repo = root.join("repo");
    fs::create_dir_all(&repo).unwrap();
    for args in [
        vec!["init", "-q", "-b", "rsi/main"],
        vec!["config", "user.name", "t"],
        vec!["config", "user.email", "t@t"],
    ] {
        git(&repo, &args).unwrap();
    }
    write(&repo.join("src/lib.rs"), "fn a() {}\n");
    git(&repo, &["add", "-A"]).unwrap();
    git(&repo, &["commit", "-q", "-m", "init"]).unwrap();
    repo
}

#[test]
fn a_git_subject_uses_worktrees_and_fast_forwards_the_branch() {
    let root = tempfile::tempdir().unwrap();
    let repo = repo(root.path());
    // Leave the branch unchecked-out in the main worktree.
    git(&repo, &["checkout", "-q", "--detach"]).unwrap();
    let subject = Subject::Git {
        repo: repo.clone(),
        branch: "rsi/main".into(),
    };
    let run = root.path().join("run");
    let incumbent = subject.incumbent(&run).unwrap();
    let first = subject.revision(&incumbent).unwrap();

    let cand = subject
        .candidate(
            &incumbent,
            "c1",
            "rsi/cand-r-e1-impl",
            &run.join("epoch-1/impl"),
        )
        .unwrap();
    write(&cand.path.join("src/lib.rs"), "fn a() { 1; }\n");
    write(&cand.path.join("tests/t.rs"), "#[test] fn t() {}\n");
    assert_eq!(
        subject.changes(&incumbent, &cand).unwrap(),
        vec!["src/lib.rs", "tests/t.rs"]
    );
    assert_eq!(git(&cand.path, &["status", "--porcelain"]).unwrap(), "");
    assert!(
        subject
            .diff(&incumbent, &cand)
            .unwrap()
            .contains("+fn a() { 1; }")
    );

    subject.promote(&incumbent, &cand).unwrap();
    let second = subject.revision(&incumbent).unwrap();
    assert_ne!(first, second);
    assert_eq!(
        git(&incumbent.path, &["rev-parse", "HEAD"]).unwrap(),
        second
    );
}

#[test]
fn promotion_merges_when_the_branch_is_checked_out_and_refuses_a_diverged_branch() {
    let root = tempfile::tempdir().unwrap();
    let repo = repo(root.path());
    let subject = Subject::Git {
        repo: repo.clone(),
        branch: "rsi/main".into(),
    };
    let run = root.path().join("run");
    let incumbent = subject.incumbent(&run).unwrap();
    let cand = subject
        .candidate(&incumbent, "c1", "rsi/cand-a", &run.join("a"))
        .unwrap();
    write(&cand.path.join("src/lib.rs"), "fn b() {}\n");
    subject.changes(&incumbent, &cand).unwrap();
    subject.promote(&incumbent, &cand).unwrap();
    assert_eq!(
        fs::read_to_string(repo.join("src/lib.rs")).unwrap(),
        "fn b() {}\n"
    );

    // A candidate made before the branch moved cannot fast-forward it.
    let stale = subject
        .candidate(&incumbent, "c2", "rsi/cand-b", &run.join("b"))
        .unwrap();
    write(&repo.join("other.rs"), "x\n");
    git(&repo, &["add", "-A"]).unwrap();
    git(&repo, &["commit", "-q", "-m", "moved"]).unwrap();
    write(&stale.path.join("src/lib.rs"), "fn c() {}\n");
    subject.changes(&incumbent, &stale).unwrap();
    assert!(matches!(
        subject.promote(&incumbent, &stale),
        Err(Error::Git { .. })
    ));
}

#[test]
fn git_errors_carry_the_last_stderr_line() {
    let root = tempfile::tempdir().unwrap();
    let err = git(root.path(), &["rev-parse", "nope"]).unwrap_err();
    assert!(matches!(err, Error::Git { args, .. } if args == "rev-parse nope"));
}

#[test]
fn a_git_spec_resolves_its_repository_and_branch() {
    let spec = SubjectSpec {
        kind: SubjectKind::Git,
        path: "/repos/memory".into(),
        branch: Some("rsi/main".into()),
        editable: vec!["src/".into()],
        frozen: vec![],
        require_test_change: true,
    };
    assert_eq!(
        Subject::from_spec(&spec, Path::new("/loops/x")),
        Subject::Git {
            repo: PathBuf::from("/repos/memory"),
            branch: "rsi/main".into()
        }
    );
}

#[test]
fn verify_finds_missing_folders_repositories_and_branches() {
    let root = tempfile::tempdir().unwrap();
    assert!(Subject::Dir { source: root.path().to_path_buf() }.verify().is_ok());
    assert!(matches!(
        Subject::Dir { source: root.path().join("none") }.verify(),
        Err(Error::Path { .. })
    ));
    let repo = repo(root.path());
    assert!(Subject::Git { repo: repo.clone(), branch: "rsi/main".into() }.verify().is_ok());
    assert!(matches!(
        Subject::Git { repo: repo.clone(), branch: "rsi/nope".into() }.verify(),
        Err(Error::Git { .. })
    ));
    assert!(Subject::Git { repo: root.path().join("none"), branch: "main".into() }
        .verify()
        .is_err());
}
