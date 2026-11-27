//! The host-run file and shell tools a process seat works with.
//!
//! A process harness runs with its built-in tools off, so a seat reads,
//! writes and runs commands only through these tools, which the host runs in
//! the seat's checkout under the seat's access: an editing seat may write in
//! its candidate, any other seat only reads the incumbent. A seat's package
//! `tools.allow` list names `OpenHuman`'s tools; each host tool stands in for
//! the names in [`STANDARD`], and an empty list gives a seat every tool its
//! access allows. Paths are relative to the checkout; an absolute path, or
//! one that climbs out with `..`, is refused.

use serde_json::{Value, json};
use tinyhivemind_lang::ExecTool;

/// One host tool: its id, the package tool names it stands in for, and
/// whether it writes.
pub(crate) struct Standard {
    /// The name the model calls.
    pub(crate) id: &'static str,
    /// The `OpenHuman` tool names, and its own, that grant it.
    pub(crate) granted_by: &'static [&'static str],
    /// Whether it changes the checkout or runs commands.
    pub(crate) writes: bool,
}

/// Every host tool a process seat may get.
pub(crate) const STANDARD: [Standard; 6] = [
    Standard {
        id: "read_file",
        granted_by: &["file_read", "read_file", "read"],
        writes: false,
    },
    Standard {
        id: "list_files",
        granted_by: &["list", "glob", "list_files"],
        writes: false,
    },
    Standard {
        id: "search_files",
        granted_by: &["grep", "search_files"],
        writes: false,
    },
    Standard {
        id: "write_file",
        granted_by: &["file_write", "write_file"],
        writes: true,
    },
    Standard {
        id: "edit_file",
        granted_by: &["edit", "apply_patch", "edit_file"],
        writes: true,
    },
    Standard {
        id: "shell",
        granted_by: &["shell"],
        writes: true,
    },
];

/// Refuses an absolute path, or one with a `..` component, before the rest
/// of a script runs; `$1` is the path.
const GUARD: &str = r#"case "$1" in /*|..|../*|*/..|*/../*) echo "outside your working directory: $1" >&2; exit 2;; esac
"#;

/// Replaces the one occurrence of `old` in the file, or fails saying how
/// many there were.
const EDIT: &str = r#"import sys
path, old, new = sys.argv[1:4]
with open(path, encoding="utf-8") as f:
    text = f.read()
count = text.count(old)
if count != 1:
    sys.exit(f"expected exactly one match of the old text in {path}, found {count}")
with open(path, "w", encoding="utf-8") as f:
    f.write(text.replace(old, new, 1))
print(f"edited {path}")
"#;

/// The host tools `allow` and `deny` give a seat that `writes` or not.
pub(crate) fn granted(allow: &[String], deny: &[String], writes: bool) -> Vec<&'static Standard> {
    let named = |names: &[String], tool: &Standard| {
        tool.granted_by
            .iter()
            .any(|grant| names.iter().any(|name| name == grant))
    };
    STANDARD
        .iter()
        .filter(|tool| writes || !tool.writes)
        .filter(|tool| allow.is_empty() || named(allow, tool))
        .filter(|tool| !named(deny, tool))
        .collect()
}

/// The executable tool declaration of `tool`.
pub(crate) fn declaration(tool: &Standard) -> ExecTool {
    let (description, command, properties, positional, timeout_ms): (
        &str,
        Value,
        Value,
        Value,
        u64,
    ) = match tool.id {
        "read_file" => (
            "Read a file in your working directory, by path relative to it.",
            json!(["sh", "-c", format!("{GUARD}cat -- \"$1\""), "read_file"]),
            json!({"path": {"type": "string"}}),
            json!(["{path}"]),
            30_000,
        ),
        "list_files" => (
            "List the files in your working directory.",
            json!([
                "sh",
                "-c",
                "find . -type f -not -path './.git/*' | sort | head -n 2000"
            ]),
            json!({}),
            json!([]),
            30_000,
        ),
        "search_files" => (
            "Search the files in your working directory for a regular expression, \
                 printing each matching line with its file and line number.",
            json!([
                "sh",
                "-c",
                "grep -rnE --exclude-dir=.git -e \"$1\" . | head -n 500",
                "search_files"
            ]),
            json!({"pattern": {"type": "string"}}),
            json!(["{pattern}"]),
            30_000,
        ),
        "write_file" => (
            "Write a file in your working directory with the full content given, \
                 replacing it if it exists.",
            json!([
                "sh",
                "-c",
                format!(
                    "{GUARD}mkdir -p \"$(dirname -- \"$1\")\" && printf '%s' \"$2\" > \"$1\" \
                         && echo \"wrote $1\""
                ),
                "write_file"
            ]),
            json!({"path": {"type": "string"}, "content": {"type": "string"}}),
            json!(["{path}", "{content}"]),
            30_000,
        ),
        "edit_file" => (
            "Replace the one occurrence of old text with new text in a file in your \
                 working directory. Fails unless the old text occurs exactly once.",
            json!([
                "sh",
                "-c",
                format!("{GUARD}exec python3 -I -c \"$0\" \"$@\""),
                EDIT
            ]),
            json!({
                "path": {"type": "string"},
                "old": {"type": "string"},
                "new": {"type": "string"}
            }),
            json!(["{path}", "{old}", "{new}"]),
            30_000,
        ),
        _ => (
            "Run a shell command in your working directory and return its output.",
            json!(["sh", "-c", "exec sh -c \"$1\" 2>&1", "shell"]),
            json!({"command": {"type": "string"}}),
            json!(["{command}"]),
            120_000,
        ),
    };
    let required: Vec<&String> = properties
        .as_object()
        .map(|a| a.keys().collect())
        .unwrap_or_default();
    let declaration = json!({
        "id": tool.id,
        "description": description,
        "command": command,
        "args": {"type": "object", "properties": properties, "required": required},
        "argv": positional,
        "stdin": "none",
        "timeout_ms": timeout_ms,
        "side_effects": if tool.writes { "write" } else { "read" },
        "cwd": "workspace",
    });
    serde_json::from_value(declaration)
        .unwrap_or_else(|error| unreachable!("a standard tool declaration is valid: {error}"))
}
