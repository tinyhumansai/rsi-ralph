//! Materializing the incumbent and candidates, finding what changed, and
//! promoting a winner.
//!
//! A `dir` subject is copied: the run keeps its own incumbent copy under
//! `<run>/incumbent/` and the loop's folder is never written. A `git` subject
//! uses worktrees of the user's repository: a detached incumbent worktree,
//! one branch and worktree per candidate (`rsi/cand-<run>-e<n>-<seat>`), and
//! promotion fast-forwards the configured branch.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use sha2::{Digest, Sha256};

use crate::{
    Error, Result,
    manifest::{SubjectKind, SubjectSpec},
};

mod types;
pub use types::{Checkout, Subject};

/// Folders never copied, hashed or compared.
const SKIPPED: [&str; 6] = [
    ".git",
    "target",
    "node_modules",
    "__pycache__",
    ".venv",
    ".pytest_cache",
];

impl Subject {
    /// The subject a manifest describes, with paths resolved against the
    /// loop folder.
    #[must_use]
    pub fn from_spec(spec: &SubjectSpec, loop_dir: &Path) -> Self {
        let path = loop_dir.join(&spec.path);
        match spec.kind {
            SubjectKind::Dir => Self::Dir { source: path },
            SubjectKind::Git => Self::Git {
                repo: path,
                branch: spec.branch.clone().unwrap_or_default(),
            },
        }
    }

    /// Materialize the incumbent at `<run>/incumbent`.
    ///
    /// # Errors
    /// Returns [`Error::Io`] or [`Error::Git`] when the checkout cannot be made.
    pub fn incumbent(&self, run_dir: &Path) -> Result<Checkout> {
        let path = run_dir.join("incumbent");
        match self {
            Self::Dir { source } => copy_tree(source, &path)?,
            Self::Git { repo, branch } => {
                git(
                    repo,
                    &["worktree", "add", "--detach", &display(&path), branch],
                )?;
            }
        }
        Ok(Checkout {
            label: "incumbent".into(),
            path,
        })
    }

    /// A content-addressed revision of the incumbent: the commit for git, a
    /// hash of every file for a folder.
    ///
    /// # Errors
    /// Returns [`Error::Io`] or [`Error::Git`] when it cannot be read.
    pub fn revision(&self, incumbent: &Checkout) -> Result<String> {
        match self {
            Self::Dir { .. } => tree_hash(&incumbent.path),
            Self::Git { repo, branch } => git(repo, &["rev-parse", branch]),
        }
    }

    /// Make a candidate checkout from the current incumbent.
    ///
    /// # Errors
    /// Returns [`Error::Io`] or [`Error::Git`] when it cannot be made.
    pub fn candidate(
        &self,
        incumbent: &Checkout,
        label: &str,
        branch: &str,
        path: &Path,
    ) -> Result<Checkout> {
        match self {
            Self::Dir { .. } => copy_tree(&incumbent.path, path)?,
            Self::Git { repo, branch: base } => {
                git(
                    repo,
                    &["worktree", "add", "-B", branch, &display(path), base],
                )?;
            }
        }
        Ok(Checkout {
            label: label.into(),
            path: path.to_owned(),
        })
    }

    /// Paths the candidate changed relative to the incumbent, sorted.
    ///
    /// A git candidate's uncommitted work is committed first, so the branch
    /// holds exactly what is judged.
    ///
    /// # Errors
    /// Returns [`Error::Io`] or [`Error::Git`] when the trees cannot be read.
    pub fn changes(&self, incumbent: &Checkout, candidate: &Checkout) -> Result<Vec<String>> {
        match self {
            Self::Dir { .. } => {
                let before = files(&incumbent.path)?;
                let after = files(&candidate.path)?;
                let mut changed: Vec<String> = before
                    .keys()
                    .chain(after.keys())
                    .filter(|p| before.get(*p) != after.get(*p))
                    .cloned()
                    .collect();
                changed.sort();
                changed.dedup();
                Ok(changed)
            }
            Self::Git { branch, .. } => {
                commit_all(&candidate.path, &candidate.label)?;
                let range = format!("{branch}...HEAD");
                let out = git(&candidate.path, &["diff", "--name-only", &range])?;
                Ok(out.lines().map(str::to_owned).collect())
            }
        }
    }

    /// A unified diff of the candidate against the incumbent, for judges and
    /// the ledger.
    ///
    /// # Errors
    /// Returns [`Error::Io`] or [`Error::Git`] when the diff cannot be made.
    pub fn diff(&self, incumbent: &Checkout, candidate: &Checkout) -> Result<String> {
        match self {
            Self::Dir { .. } => {
                let output = Command::new("diff")
                    .args(["-ruN", "-x", "__pycache__"])
                    .arg(&incumbent.path)
                    .arg(&candidate.path)
                    .output()
                    .map_err(|e| Error::io("diff", &candidate.path, e))?;
                Ok(String::from_utf8_lossy(&output.stdout).into_owned())
            }
            Self::Git { branch, .. } => {
                git(&candidate.path, &["diff", &format!("{branch}...HEAD")])
            }
        }
    }

    /// Make the candidate the incumbent.
    ///
    /// # Errors
    /// Returns [`Error::Git`] when the branch cannot be fast-forwarded (it
    /// moved underneath the run) and [`Error::Io`] when a copy fails.
    pub fn promote(&self, incumbent: &Checkout, candidate: &Checkout) -> Result<()> {
        match self {
            Self::Dir { .. } => {
                fs::remove_dir_all(&incumbent.path)
                    .map_err(|e| Error::io("remove", &incumbent.path, e))?;
                copy_tree(&candidate.path, &incumbent.path)
            }
            Self::Git { repo, branch } => {
                let head = git(&candidate.path, &["rev-parse", "HEAD"])?;
                let current = git(repo, &["rev-parse", branch]).ok();
                git(repo, &["merge-base", "--is-ancestor", branch, &head])?;
                let checked_out = git(repo, &["symbolic-ref", "-q", "HEAD"])
                    .is_ok_and(|r| r == format!("refs/heads/{branch}"));
                if checked_out {
                    git(repo, &["merge", "--ff-only", &head])?;
                } else {
                    let reference = format!("refs/heads/{branch}");
                    let mut args = vec!["update-ref", reference.as_str(), head.as_str()];
                    if let Some(old) = &current {
                        args.push(old);
                    }
                    git(repo, &args)?;
                }
                git(&incumbent.path, &["checkout", "--detach", &head]).map(|_| ())
            }
        }
    }
}

fn display(path: &Path) -> String {
    path.display().to_string()
}

/// Run git in `dir` and return trimmed stdout.
pub(crate) fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|e| Error::io("run git in", dir, e))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(Error::Git {
            args: args.join(" "),
            stderr: stderr
                .lines()
                .map(str::trim)
                .rfind(|l| !l.is_empty())
                .unwrap_or("failed")
                .to_owned(),
        })
    }
}

/// Commit every change in a worktree, if there is any.
fn commit_all(dir: &Path, label: &str) -> Result<()> {
    if git(dir, &["status", "--porcelain"])?.is_empty() {
        return Ok(());
    }
    git(dir, &["add", "-A"])?;
    git(
        dir,
        &[
            "-c",
            "user.name=tinyrsi",
            "-c",
            "user.email=tinyrsi@localhost",
            "commit",
            "-q",
            "--no-verify",
            "-m",
            &format!("rsi: uncommitted work of {label}"),
        ],
    )
    .map(|_| ())
}

/// Copy a folder, skipping symlinks and [`SKIPPED`] folders.
pub(crate) fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to).map_err(|e| Error::io("create", to, e))?;
    for entry in fs::read_dir(from).map_err(|e| Error::io("read", from, e))? {
        let entry = entry.map_err(|e| Error::io("read", from, e))?;
        let kind = entry
            .file_type()
            .map_err(|e| Error::io("stat", entry.path(), e))?;
        let name = entry.file_name();
        let target = to.join(&name);
        if kind.is_dir() {
            if !SKIPPED.contains(&name.to_string_lossy().as_ref()) {
                copy_tree(&entry.path(), &target)?;
            }
        } else if kind.is_file() {
            fs::copy(entry.path(), &target).map_err(|e| Error::io("copy", entry.path(), e))?;
        }
    }
    Ok(())
}

/// Every file under `root` by `/`-separated relative path, with its bytes.
fn files(root: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) -> Result<()> {
        for entry in fs::read_dir(dir).map_err(|e| Error::io("read", dir, e))? {
            let entry = entry.map_err(|e| Error::io("read", dir, e))?;
            let path: PathBuf = entry.path();
            let kind = entry.file_type().map_err(|e| Error::io("stat", &path, e))?;
            if kind.is_dir() {
                if !SKIPPED.contains(&entry.file_name().to_string_lossy().as_ref()) {
                    walk(root, &path, out)?;
                }
            } else if kind.is_file() {
                let bytes = fs::read(&path).map_err(|e| Error::io("read", &path, e))?;
                let relative = path
                    .strip_prefix(root)
                    .map_or_else(|_| path.clone(), Path::to_path_buf)
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/");
                out.insert(relative, bytes);
            }
        }
        Ok(())
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out)?;
    Ok(out)
}

/// First 12 hex digits of SHA-256 over every relative path and content.
fn tree_hash(root: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    for (path, bytes) in files(root)? {
        hasher.update(path.as_bytes());
        hasher.update([0]);
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
    }
    let digest = hasher.finalize();
    Ok(digest.iter().take(6).map(|b| format!("{b:02x}")).collect())
}

#[cfg(test)]
mod test;
