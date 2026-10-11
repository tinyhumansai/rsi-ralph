//! Unit tests for seat system prompts.

use super::*;

fn parts<'a>(context: &'a BTreeMap<String, String>, editing: bool) -> PromptParts<'a> {
    PromptParts {
        seat: "  You implement the assignment.\n",
        role: "implementer",
        charter: "Makes the change.\n",
        action_dir: Path::new("/runs/epoch-1/candidates/implementer-a"),
        editing,
        context,
    }
}

#[test]
fn composes_seat_role_directory_context_and_rules_in_order() {
    let context = BTreeMap::from([
        ("context/b.md".to_owned(), "- second\n".to_owned()),
        ("context/a.md".to_owned(), "- first\n".to_owned()),
    ]);
    let text = system_prompt(&parts(&context, true));
    assert!(text.starts_with("You implement the assignment.\n\n## Your role: implementer\n\nMakes the change.\n"));
    assert!(text.contains(
        "## Your working directory\n\n/runs/epoch-1/candidates/implementer-a (your own candidate checkout; edit it freely)\n"
    ));
    let first = text.find("## Shared context: context/a.md\n\n- first").unwrap_or(usize::MAX);
    let second = text.find("## Shared context: context/b.md\n\n- second").unwrap_or(0);
    assert!(first < second);
    assert!(text.ends_with(HIVE_RULES));
}

#[test]
fn read_only_seats_are_told_their_checkout_is_the_incumbent() {
    let text = system_prompt(&parts(&BTreeMap::new(), false));
    assert!(text.contains("(the incumbent checkout; read-only)"));
    assert!(!text.contains("Shared context"));
}

#[test]
fn the_rules_forbid_waiting_inside_a_turn() {
    assert!(HIVE_RULES.contains("END YOUR TURN"));
    assert!(HIVE_RULES.contains("Never wait, sleep or poll"));
    assert!(HIVE_RULES.contains("hivemind_send_agent"));
}
