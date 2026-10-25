//! The harvest's input and its stored index, `epoch-<n>/notes.json`.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// One candidate whose notes may be harvested.
#[derive(Debug, Clone, Copy)]
pub struct Source<'a> {
    /// Seat that edited it.
    pub seat: &'a str,
    /// Candidate id, `e<epoch>-<seat>`.
    pub candidate: &'a str,
    /// Its outcome in a word or two, as `{{history}}` gives it.
    pub outcome: &'a str,
    /// Its checkout.
    pub checkout: &'a Path,
    /// Paths it changed relative to the epoch's incumbent.
    pub changed: &'a [String],
}

/// What one epoch's seats passed on: the whole of `epoch-<n>/notes.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpochNotes {
    /// Epoch the notes were written in.
    pub epoch: u32,
    /// Candidate promoted at the end of that epoch, if any.
    pub promoted: Option<String>,
    /// Seats that changed at least one carried file, by seat id.
    pub seats: Vec<SeatNotes>,
}

/// One seat's harvested notes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeatNotes {
    /// Seat id; its files sit under `epoch-<n>/notes/<seat>/`.
    pub seat: String,
    /// Its candidate id.
    pub candidate: String,
    /// Its candidate's outcome, such as `promoted` or `rejected`.
    pub outcome: String,
    /// Its carried files, by path.
    pub files: Vec<NoteFile>,
}

/// One harvested file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteFile {
    /// Path in the candidate checkout, `/`-separated.
    pub path: String,
    /// Its size in the checkout.
    pub bytes: usize,
    /// Bytes kept in the store: less than `bytes` when a cap cut it, 0 when
    /// the epoch's cap left no room for it.
    pub kept: usize,
}
