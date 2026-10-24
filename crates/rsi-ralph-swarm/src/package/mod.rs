//! Hive packages: reading one from disk, and the per-seat facts the swarm needs.
//!
//! The hive language opens no files. A host hands it every package file as a
//! relative path → text map; [`load_package`] builds that map from a folder,
//! and [`Hive::open`] parses, validates and lowers it, keeping each seat's
//! role charter beside the lowered seat.

mod types;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::{Error, Result};
pub(crate) use types::{Hive, SeatPlan};

/// `config.action_dir` of a seat that edits its own candidate checkout.
pub const CANDIDATE_DIR: &str = "{candidate}";

/// Reads the hive package rooted at `dir` into the path → text map the hive
/// language parses, and checks that it is a valid package.
///
/// Kept: the manifests at the package root (`hive.json`,
/// `constitution.json`, `memory.json` and any other root `*.json`), and
/// every Markdown document below a subdirectory (`roles/`, `seats/`,
/// `context/`, …). Skipped: hidden entries, symbolic links, root Markdown
/// files and `README.md` indexes (matched case-insensitively), which document
/// the package without being part of it. Keys use `/` separators on every
/// platform.
///
/// # Errors
///
/// [`Error::PackageIo`] when a directory or file cannot be read,
/// [`Error::PackageEncoding`] for a non-UTF-8 name or file, and
/// [`Error::Package`] when the files do not form a valid hive package.
pub fn load_package(dir: &Path) -> Result<BTreeMap<String, String>> {
    let mut files = BTreeMap::new();
    collect(dir, &mut Vec::new(), &mut files)?;
    Hive::open(&files)?;
    Ok(files)
}

/// The seats whose `config.action_dir` is `"{candidate}"`: each needs its
/// own editable checkout in `SwarmRequest::candidates`. Sorted by seat id.
///
/// # Errors
///
/// [`Error::Package`] when `package` is not a valid hive package.
pub fn editing_seats(package: &BTreeMap<String, String>) -> Result<Vec<String>> {
    let hive = Hive::open(package)?;
    let mut seats: Vec<String> = hive
        .seats
        .iter()
        .filter(|seat| seat.editing)
        .map(|seat| seat.seat.id.clone())
        .collect();
    seats.sort();
    Ok(seats)
}

/// Walks `dir` depth-first, adding kept files under their relative `prefix`.
fn collect(
    dir: &Path,
    prefix: &mut Vec<String>,
    files: &mut BTreeMap<String, String>,
) -> Result<()> {
    let mut entries = std::fs::read_dir(dir)
        .map_err(package_io(dir))?
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(package_io(dir))?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| Error::PackageEncoding { path: path.clone() })?;
        if name.starts_with('.') {
            continue;
        }
        let kind = entry.file_type().map_err(package_io(&path))?;
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            prefix.push(name);
            collect(&path, prefix, files)?;
            prefix.pop();
        } else if kind.is_file() && keeps(prefix.is_empty(), &name) {
            files.insert(key(prefix, &name), read(path)?);
        }
    }
    Ok(())
}

/// Whether a file named `name` belongs in the package map.
fn keeps(at_root: bool, name: &str) -> bool {
    let extension = Path::new(name)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("json") => at_root,
        Some("md") => !at_root && !name.eq_ignore_ascii_case("readme.md"),
        _ => false,
    }
}

fn key(prefix: &[String], name: &str) -> String {
    let mut parts = prefix.to_vec();
    parts.push(name.to_owned());
    parts.join("/")
}

/// Wrap an I/O failure on `path` as [`Error::PackageIo`].
fn package_io(path: &Path) -> impl FnOnce(std::io::Error) -> Error + use<> {
    let path = path.to_path_buf();
    move |source| Error::PackageIo { path, source }
}

fn read(path: PathBuf) -> Result<String> {
    let bytes = std::fs::read(&path).map_err(package_io(&path))?;
    String::from_utf8(bytes).map_err(|_| Error::PackageEncoding { path })
}

#[cfg(test)]
mod test;
