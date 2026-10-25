//! Harvesting seats' notes into the store, the size caps, and the brief text.
#![expect(clippy::unwrap_used, reason = "assert deterministic fixtures")]
use super::*;
use serde_json::json;
use std::fs;

fn carry(file_bytes: usize, epoch_bytes: usize) -> Carry {
    Carry {
        notes: vec!["notes/".into(), "log.md".into()],
        file_bytes,
        epoch_bytes,
        brief_chars: 24_000,
    }
}

/// A candidate checkout holding `files`, all of which it changed.
struct Seat {
    seat: &'static str,
    outcome: &'static str,
    dir: tempfile::TempDir,
    changed: Vec<String>,
}

fn seat(seat: &'static str, outcome: &'static str, files: &[(&str, &str)]) -> Seat {
    let dir = tempfile::tempdir().unwrap();
    for (path, text) in files {
        let target = dir.path().join(path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, text).unwrap();
    }
    Seat {
        seat,
        outcome,
        dir,
        changed: files.iter().map(|(p, _)| (*p).to_owned()).collect(),
    }
}

fn sources(seats: &[Seat]) -> Vec<(String, Source<'_>)> {
    seats
        .iter()
        .map(|s| {
            let id = format!("e1-{}", s.seat);
            (
                id,
                Source {
                    seat: s.seat,
                    candidate: "",
                    outcome: s.outcome,
                    checkout: s.dir.path(),
                    changed: &s.changed,
                },
            )
        })
        .collect()
}

fn run(spec: &Carry, seats: &[Seat], promoted: Option<&str>) -> (tempfile::TempDir, EpochNotes) {
    let ids = sources(seats);
    let list: Vec<Source<'_>> = ids
        .iter()
        .map(|(id, s)| Source {
            candidate: id,
            ..*s
        })
        .collect();
    let epoch = tempfile::tempdir().unwrap();
    let notes = harvest(spec, 1, promoted, &list, epoch.path()).unwrap();
    (epoch, notes)
}

#[test]
fn harvests_only_changed_files_under_the_carry_paths() {
    let mut reducer = seat(
        "reducer",
        "promoted",
        &[("notes/reducer.md", "reduce to Rung2"), ("src/x.lean", "x")],
    );
    // A file the seat did not change, such as the README every checkout
    // starts with, is not the seat's note.
    fs::write(reducer.dir.path().join("notes/README.md"), "shared").unwrap();
    // A deleted note is listed as changed but has nothing to carry.
    reducer.changed.push("notes/gone.md".into());
    let dens = seat(
        "dens-a",
        "rejected",
        &[("notes/dens-a.md", "density fails at 7"), ("log.md", "tried 3 bounds")],
    );
    let (epoch, notes) = run(&carry(1000, 10_000), &[dens, reducer], Some("e1-reducer"));

    assert_eq!(notes.epoch, 1);
    assert_eq!(notes.promoted.as_deref(), Some("e1-reducer"));
    let files: Vec<(&str, Vec<&str>)> = notes
        .seats
        .iter()
        .map(|s| (s.seat.as_str(), s.files.iter().map(|f| f.path.as_str()).collect()))
        .collect();
    assert_eq!(
        files,
        vec![
            ("dens-a", vec!["log.md", "notes/dens-a.md"]),
            ("reducer", vec!["notes/reducer.md"]),
        ]
    );
    assert_eq!(notes.seats[0].candidate, "e1-dens-a");
    assert_eq!(notes.seats[0].outcome, "rejected");
    assert_eq!(
        fs::read_to_string(epoch.path().join("notes/dens-a/notes/dens-a.md")).unwrap(),
        "density fails at 7"
    );
    assert!(!epoch.path().join("notes/reducer/notes/README.md").exists());
    let index: EpochNotes = crate::store::read_json(&epoch.path().join("notes.json")).unwrap();
    assert_eq!(index, notes);
}

#[test]
fn cuts_a_file_to_the_file_cap_on_a_character_boundary() {
    // "é" is two bytes, so a five-byte cut of "aéé" keeps "aé" and not half a character.
    let a = seat("a", "rejected", &[("notes/a.md", "aééé")]);
    let (epoch, notes) = run(&carry(4, 10_000), &[a], None);
    let file = &notes.seats[0].files[0];
    assert_eq!((file.bytes, file.kept), (7, 3));
    assert_eq!(
        fs::read_to_string(epoch.path().join("notes/a/notes/a.md")).unwrap(),
        "aé"
    );
}

#[test]
fn shares_the_epoch_cap_so_one_seat_cannot_crowd_out_the_rest() {
    let flood = seat(
        "flood",
        "rejected",
        &[("notes/1.md", &"x".repeat(80)), ("notes/2.md", &"y".repeat(80))],
    );
    let quiet = seat("quiet", "rejected", &[("notes/q.md", "small but vital")]);
    let (epoch, notes) = run(&carry(1000, 100), &[flood, quiet], None);
    let kept: Vec<Vec<(usize, usize)>> = notes
        .seats
        .iter()
        .map(|s| s.files.iter().map(|f| (f.bytes, f.kept)).collect())
        .collect();
    // quiet keeps its 15 bytes; flood gets the other 85, its second file cut
    // to 5 bytes.
    assert_eq!(kept, vec![vec![(80, 80), (80, 5)], vec![(15, 15)]]);
    let total: usize = notes.seats.iter().flat_map(|s| &s.files).map(|f| f.kept).sum();
    assert!(total <= 100);
    assert_eq!(
        fs::read_to_string(epoch.path().join("notes/flood/notes/2.md")).unwrap(),
        "yyyyy"
    );
}

#[test]
fn water_filling_gives_small_seats_all_they_need_and_splits_the_rest() {
    assert_eq!(shares(&[10, 500, 500], 310), vec![10, 150, 150]);
    assert_eq!(shares(&[10, 20], 100), vec![10, 20]);
    assert_eq!(shares(&[], 100), Vec::<usize>::new());
    // Integer division leaves a remainder for the last seat, not on the floor.
    assert_eq!(shares(&[7, 7, 7], 10), vec![3, 3, 4]);
}

#[test]
fn an_epoch_where_no_seat_wrote_notes_harvests_nothing_and_renders_nothing() {
    let a = seat("a", "rejected", &[("src/a.py", "code only")]);
    let empty = seat("b", "empty", &[]);
    let (epoch, notes) = run(&carry(1000, 1000), &[a, empty], Some("e1-a"));
    assert!(notes.seats.is_empty());
    assert!(epoch.path().join("notes.json").exists());
    assert!(!epoch.path().join("notes").exists());
    assert_eq!(render(epoch.path(), &notes, 24_000), "");
}

#[test]
fn renders_one_section_per_seat_and_names_the_promoted_candidate() {
    let reducer = seat("reducer", "promoted", &[("notes/reducer.md", "Rung2 reduces")]);
    let dens = seat("dens-a", "rejected", &[("notes/dens-a.md", "density fails at 7")]);
    let (epoch, notes) = run(&carry(1000, 10_000), &[reducer, dens], Some("e1-reducer"));
    let out = render(epoch.path(), &notes, 24_000);
    assert!(out.contains("epoch 1 promoted `e1-reducer`"), "{out}");
    assert!(
        out.contains("### `dens-a` (`e1-dens-a`, rejected)\n\n#### `notes/dens-a.md`\n\ndensity fails at 7"),
        "{out}"
    );
    assert!(out.contains("### `reducer` (`e1-reducer`, promoted)"), "{out}");
    assert!(out.find("dens-a").unwrap() < out.find("### `reducer`").unwrap());

    let (epoch, notes) = run(
        &carry(1000, 10_000),
        &[seat("dens-a", "rejected", &[("notes/d.md", "d")])],
        None,
    );
    assert!(render(epoch.path(), &notes, 24_000).contains("epoch 1 promoted no candidate"));
}

#[test]
fn rendering_fits_the_brief_budget_and_marks_every_cut() {
    let big = seat("big", "rejected", &[("notes/big.md", &"b".repeat(5000))]);
    let small = seat("small", "rejected", &[("notes/s.md", "keep me whole")]);
    let (epoch, notes) = run(&carry(1000, 10_000), &[big, small], None);
    // The harvest itself cut big's note to 1,000 bytes; say so.
    let out = render(epoch.path(), &notes, 24_000);
    assert!(out.contains("(cut to 1000 of 5000 bytes)"), "{out}");

    let out = render(epoch.path(), &notes, 600);
    assert!(out.contains("keep me whole"), "{out}");
    let bodies: usize = out.matches('b').count();
    assert!(bodies < 600, "{bodies}");
    assert!(out.contains('…'), "{out}");
}

#[test]
fn a_file_the_epoch_cap_dropped_is_named_in_the_brief() {
    let flood = seat(
        "flood",
        "rejected",
        &[("notes/1.md", &"x".repeat(50)), ("notes/2.md", "late")],
    );
    let (epoch, notes) = run(&carry(1000, 50), &[flood], None);
    assert_eq!(notes.seats[0].files[1].kept, 0);
    assert!(!epoch.path().join("notes/flood/notes/2.md").exists());
    let out = render(epoch.path(), &notes, 24_000);
    assert!(out.contains("#### `notes/2.md`\n\n(dropped: the epoch's notes cap was reached)"), "{out}");
}

#[test]
fn pins_the_stored_form_of_the_notes_index() {
    let notes = EpochNotes {
        epoch: 2,
        promoted: Some("e2-a".into()),
        seats: vec![SeatNotes {
            seat: "a".into(),
            candidate: "e2-a".into(),
            outcome: "promoted".into(),
            files: vec![NoteFile {
                path: "notes/a.md".into(),
                bytes: 10,
                kept: 8,
            }],
        }],
    };
    let stored = serde_json::to_value(&notes).unwrap();
    assert_eq!(
        stored,
        json!({
            "epoch": 2,
            "promoted": "e2-a",
            "seats": [{
                "seat": "a", "candidate": "e2-a", "outcome": "promoted",
                "files": [{ "path": "notes/a.md", "bytes": 10, "kept": 8 }]
            }]
        })
    );
    let back: EpochNotes = serde_json::from_value(stored).unwrap();
    assert_eq!(back, notes);
}
