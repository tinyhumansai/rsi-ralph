//! Carrying every seat's notes into the next epoch.
//!
//! Only the promoted candidate's checkout becomes the next incumbent, so
//! whatever the other seats wrote would end with the epoch. With a `carry`
//! section in `rsi.json`, [`harvest`] runs once the epoch has promoted (or
//! not): each candidate's changed files under a `carry.notes` prefix are
//! copied into the run store,
//!
//! ```text
//! epoch-<n>/notes.json                  the index, EpochNotes
//! epoch-<n>/notes/<seat>/<path>         each kept file
//! ```
//!
//! and [`render`] turns them into the next brief's `{{notes}}`, one section
//! per seat naming its candidate and outcome.
//!
//! Only changed files count. A note a seat left alone is the incumbent's,
//! not the seat's, so it is not harvested again in every seat's name.
//!
//! Three caps keep one seat from flooding the store or the brief: each file
//! is cut to `file_bytes`; the epoch keeps at most `epoch_bytes`; a brief
//! shows at most `brief_chars` of notes text (headings are extra). The two
//! shared caps are water-filled across seats: a seat that needs less than an
//! even share keeps all of it, and the rest is split among the others.
//!
//! Notes are not written into the next incumbent's checkout. A scorer that
//! reads notes would then credit each candidate with every other seat's
//! findings, and a `git` subject's candidates are branched from the
//! incumbent's branch, so files placed in its worktree would not reach them.

use std::{fmt::Write as _, fs, path::Path};

use crate::{Result, brief::clip, gate::under, manifest::Carry, store};

mod types;
pub use types::{EpochNotes, NoteFile, SeatNotes, Source};

/// The index file in an epoch folder.
pub const INDEX: &str = "notes.json";

/// The folder of kept files in an epoch folder.
pub const FOLDER: &str = "notes";

/// Copy each source's carried files into `epoch_dir` and write the index.
///
/// Files that are gone (deleted by the seat) or not UTF-8 text are skipped.
/// The index is written even when no seat wrote notes.
///
/// # Errors
/// Returns [`crate::Error::Path`] for a seat id or changed path that is not a
/// plain relative path, and [`crate::Error::Io`] when the store cannot be
/// written.
pub fn harvest(
    carry: &Carry,
    epoch: u32,
    promoted: Option<&str>,
    sources: &[Source<'_>],
    epoch_dir: &Path,
) -> Result<EpochNotes> {
    let mut found: Vec<(Source<'_>, Vec<(String, String)>)> = Vec::new();
    for source in sources {
        let files = carried(carry, source)?;
        if !files.is_empty() {
            found.push((*source, files));
        }
    }
    found.sort_by(|a, b| a.0.seat.cmp(b.0.seat));
    let sizes: Vec<usize> = found
        .iter()
        .map(|(_, files)| {
            files
                .iter()
                .map(|(_, t)| cut(t, carry.file_bytes).len())
                .sum()
        })
        .collect();
    let mut notes = EpochNotes {
        epoch,
        promoted: promoted.map(str::to_owned),
        seats: Vec::new(),
    };
    for ((source, files), share) in found.iter().zip(shares(&sizes, carry.epoch_bytes)) {
        crate::manifest::plain(source.seat)?;
        let mut room = share;
        let mut kept_files = Vec::new();
        for (path, text) in files {
            let kept = cut(cut(text, carry.file_bytes), room);
            room -= kept.len();
            if !kept.is_empty() {
                let target = epoch_dir.join(FOLDER).join(source.seat).join(path);
                store::write_atomic(&target, kept.as_bytes())?;
            }
            kept_files.push(NoteFile {
                path: path.clone(),
                bytes: text.len(),
                kept: kept.len(),
            });
        }
        notes.seats.push(SeatNotes {
            seat: source.seat.to_owned(),
            candidate: source.candidate.to_owned(),
            outcome: source.outcome.to_owned(),
            files: kept_files,
        });
    }
    store::write_json(&epoch_dir.join(INDEX), &notes)?;
    Ok(notes)
}

/// The source's changed files under a carry prefix, by path, with their text.
fn carried(carry: &Carry, source: &Source<'_>) -> Result<Vec<(String, String)>> {
    let mut paths: Vec<&String> = source
        .changed
        .iter()
        .filter(|path| carry.notes.iter().any(|prefix| under(path, prefix)))
        .collect();
    paths.sort();
    paths.dedup();
    let mut files = Vec::new();
    for path in paths {
        crate::manifest::plain(path)?;
        let full = source.checkout.join(path);
        if !full.is_file() {
            continue;
        }
        if let Ok(text) = fs::read_to_string(&full) {
            files.push((path.clone(), text));
        }
    }
    Ok(files)
}

/// The longest prefix of `text` of at most `bytes` bytes that ends on a
/// character boundary.
fn cut(text: &str, bytes: usize) -> &str {
    let mut end = bytes.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// Water-fill `budget` over `sizes`: each gets what it needs or an even share
/// of what is left, smallest first. The shares never sum past `budget`.
fn shares(sizes: &[usize], budget: usize) -> Vec<usize> {
    let mut order: Vec<usize> = (0..sizes.len()).collect();
    order.sort_by_key(|&i| sizes[i]);
    let mut out = vec![0; sizes.len()];
    let mut left = budget;
    for (done, &i) in order.iter().enumerate() {
        let share = sizes[i].min(left / (sizes.len() - done));
        out[i] = share;
        left -= share;
    }
    out
}

/// The `{{notes}}` text for an epoch's harvest, or nothing when no seat
/// wrote notes. Kept files are read back from `epoch_dir`.
#[must_use]
pub fn render(epoch_dir: &Path, notes: &EpochNotes, budget: usize) -> String {
    if notes.seats.is_empty() {
        return String::new();
    }
    let epoch = notes.epoch;
    let mut out = match &notes.promoted {
        Some(id) => format!(
            "The notes each seat changed in epoch {epoch}. Only the candidate epoch {epoch} \
             promoted `{id}` reached your checkout; the other seats' notes are only here.\n\n"
        ),
        None => format!(
            "The notes each seat changed in epoch {epoch}. Since epoch {epoch} promoted no \
             candidate, you start from the same incumbent, and these notes are only here.\n\n"
        ),
    };
    let bodies: Vec<String> = notes
        .seats
        .iter()
        .map(|seat| seat_body(epoch_dir, seat))
        .collect();
    let sizes: Vec<usize> = bodies.iter().map(|b| b.chars().count()).collect();
    for ((seat, body), share) in notes.seats.iter().zip(&bodies).zip(shares(&sizes, budget)) {
        let _ = write!(
            out,
            "### `{}` (`{}`, {})\n\n{}\n\n",
            seat.seat,
            seat.candidate,
            seat.outcome,
            clip(body, share).trim_end()
        );
    }
    out
}

/// One seat's files as Markdown, each under its path.
fn seat_body(epoch_dir: &Path, seat: &SeatNotes) -> String {
    let mut out = String::new();
    for file in &seat.files {
        let _ = write!(out, "#### `{}`\n\n", file.path);
        if file.kept == 0 {
            out.push_str("(dropped: the epoch's notes cap was reached)\n\n");
            continue;
        }
        let path = epoch_dir.join(FOLDER).join(&seat.seat).join(&file.path);
        let text = fs::read_to_string(path).unwrap_or_default();
        let _ = write!(out, "{}\n\n", text.trim_end());
        if file.kept < file.bytes {
            let _ = write!(out, "(cut to {} of {} bytes)\n\n", file.kept, file.bytes);
        }
    }
    out
}

#[cfg(test)]
mod test;
