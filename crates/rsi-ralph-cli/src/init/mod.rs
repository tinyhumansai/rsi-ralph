//! `rsi-ralph init`: write a starter loop folder.
//!
//! An installed binary has no checkout to copy examples from, so the bundled
//! templates are compiled in with `include_str!` from `loops/`. Each template
//! is a complete loop that passes `rsi-ralph check` once `OPENROUTER_API_KEY` is
//! set. The new folder's `rsi.json` takes the folder's name, so runs of two
//! loops made from one template do not share a run-store directory.

use std::path::Path;

use crate::{Error, Result, manifest};

/// One bundled loop: a name and its files as relative path → text.
#[derive(Debug, Clone, Copy)]
pub struct Template {
    /// The name `--template` takes.
    pub name: &'static str,
    /// One line describing what the loop improves.
    pub about: &'static str,
    files: &'static [(&'static str, &'static str)],
}

macro_rules! loop_files {
    ($root:literal: $($path:literal),+ $(,)?) => {
        &[$(($path, include_str!(concat!("../../../../loops/", $root, "/", $path)))),+]
    };
}

/// Every template, the default first.
pub const TEMPLATES: &[Template] = &[
    Template {
        name: "graph-coloring",
        about: "colour random graphs with as few colours as possible (Python scorer)",
        files: loop_files!("graph-coloring":
            "rsi.json",
            "brief.md",
            "scorers/colors.py",
            "subject/coloring.py",
            "hive/hive.json",
            "hive/constitution.json",
            "hive/memory.json",
            "hive/roles/lead.md",
            "hive/roles/implementer.md",
            "hive/context/rules.md",
            "hive/seats/lead.md",
            "hive/seats/implementer.md",
        ),
    },
    Template {
        name: "math-problem",
        about: "compute pi to 12 digits, with an LLM judge of the method (Python scorer)",
        files: loop_files!("math-problem":
            "rsi.json",
            "brief.md",
            "scorers/digits.py",
            "scorers/justification.md",
            "subject/pi.py",
            "hive/hive.json",
            "hive/constitution.json",
            "hive/memory.json",
            "hive/roles/lead.md",
            "hive/roles/implementer.md",
            "hive/context/rules.md",
            "hive/seats/lead.md",
            "hive/seats/implementer.md",
        ),
    },
    Template {
        name: "upgrade-file",
        about: "bring a file to a coding standard behind a behavior gate (Python gate and scorer)",
        files: loop_files!("upgrade-file":
            "rsi.json",
            "brief.md",
            "gates/behavior.py",
            "scorers/violations.py",
            "scorers/readability.md",
            "subject/slug.py",
            "hive/hive.json",
            "hive/constitution.json",
            "hive/memory.json",
            "hive/roles/lead.md",
            "hive/roles/implementer.md",
            "hive/context/rules.md",
            "hive/seats/lead.md",
            "hive/seats/implementer.md",
        ),
    },
];

/// The template named `name`.
///
/// # Errors
/// [`Error::Reference`] when no bundled template has that name.
pub fn template(name: &str) -> Result<&'static Template> {
    TEMPLATES
        .iter()
        .find(|t| t.name == name)
        .ok_or_else(|| Error::Reference {
            kind: "template",
            id: name.to_owned(),
        })
}

/// Write `template` into `dir`, creating it, and return the loop's name.
///
/// `dir` must not exist or be empty, so `init` never overwrites a file.
/// The loop is named after `dir`'s last component.
///
/// # Errors
/// [`Error::NotEmpty`] when `dir` already holds files, [`Error::Field`] when
/// its name is not a usable loop name, and [`Error::Io`] when a file cannot
/// be written.
pub fn write(template: &Template, dir: &Path) -> Result<String> {
    let name = loop_name(dir)?;
    if dir
        .read_dir()
        .is_ok_and(|mut entries| entries.next().is_some())
    {
        return Err(Error::NotEmpty(dir.to_path_buf()));
    }
    for (path, text) in template.files {
        let text = if *path == manifest::MANIFEST {
            renamed(text, &name)?
        } else {
            (*text).to_owned()
        };
        let target = dir.join(path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::io("create", parent, e))?;
        }
        std::fs::write(&target, text).map_err(|e| Error::io("write", &target, e))?;
    }
    Ok(name)
}

/// The loop name for `dir`: its last component, which must be a plain name.
fn loop_name(dir: &Path) -> Result<String> {
    dir.file_name()
        .and_then(|n| n.to_str())
        .filter(|n| !n.is_empty() && !n.starts_with('.'))
        .map(str::to_owned)
        .ok_or_else(|| Error::field("init", format!("{} is not a loop name", dir.display())))
}

/// `rsi.json` with its `name` set to `name`, keeping the rest of the text.
fn renamed(manifest: &str, name: &str) -> Result<String> {
    let mut value: serde_json::Value = serde_json::from_str(manifest).map_err(|e| Error::Json {
        message: e.to_string(),
    })?;
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "name".to_owned(),
            serde_json::Value::String(name.to_owned()),
        );
    }
    let text = serde_json::to_string_pretty(&value).map_err(|e| Error::Json {
        message: e.to_string(),
    })?;
    Ok(text + "\n")
}

#[cfg(test)]
mod test;
