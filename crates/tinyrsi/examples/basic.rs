//! Minimal end-to-end usage of the crate.
//!
//! Examples are compiled and linted in CI, so they cannot drift from the API.
//! Run it with:
//!
//! ```sh
//! cargo run --example basic
//! ```

use tinyrsi::{Error, GradeEntry, Lineage, Verdict};

fn main() -> tinyrsi::Result<()> {
    let mut lineage = Lineage::new();

    // The baseline incumbent, then one rejected candidate and one accepted one.
    let steps = [
        (0, 0.703, Verdict::Accepted, "baseline"),
        (1, 0.5, Verdict::Rejected, "public gain, private loss"),
        (
            2,
            0.778,
            Verdict::Accepted,
            "bandit selection over strategies",
        ),
    ];
    for (step, grade, verdict, rationale) in steps {
        let entry = GradeEntry::new(step, "sha256:abc", grade, verdict, rationale);
        lineage.append(entry)?;
    }

    match lineage.head() {
        Some(incumbent) => println!(
            "incumbent is step {} at grade {:.3} ({})",
            incumbent.step, incumbent.grade, incumbent.rationale
        ),
        None => println!("no incumbent yet"),
    }

    // Failure modes are part of the public contract; show them too.
    let replay = GradeEntry::new(0, "sha256:abc", 0.5, Verdict::Accepted, "replay");
    match lineage.append(replay) {
        Ok(recorded) => println!("recorded: {recorded}"),
        Err(Error::StepOutOfOrder) => println!("expected failure: replayed step rejected"),
        Err(Error::EmptyField) => println!("expected failure: empty field rejected"),
        Err(_) => println!("unexpected failure"),
    }

    Ok(())
}
