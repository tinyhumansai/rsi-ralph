//! Secret values for the keys a loop names.
//!
//! `rsi.json` only ever holds variable *names*. [`Keys::resolve`] reads each
//! value from the process environment, falling back to a `.env` file in the
//! loop folder, and the resulting [`Secret`]s never print: their `Debug` is
//! redacted and nothing in the store, the trace or a prompt receives them.

use std::{collections::BTreeMap, fmt, path::Path};

use crate::{Error, Result, manifest::Loop};

/// A key value that never appears in debug output.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    /// The value, for the one caller that must send it.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(***)")
    }
}

/// Resolved values for every variable a loop needs.
#[derive(Debug, Clone, Default)]
pub struct Keys {
    values: BTreeMap<String, Secret>,
}

impl Keys {
    /// Resolve every variable [`Loop::required_env`] names.
    ///
    /// `lookup` reads the process environment (tests pass a map). A `.env`
    /// in `loop_dir` fills in what the environment lacks.
    ///
    /// # Errors
    /// Returns [`Error::MissingKeys`] naming every variable with no value,
    /// and [`Error::Io`] when an existing `.env` cannot be read.
    pub fn resolve(
        parsed: &Loop,
        loop_dir: &Path,
        lookup: &dyn Fn(&str) -> Option<String>,
    ) -> Result<Self> {
        let dotenv = read_dotenv(&loop_dir.join(".env"))?;
        let mut values = BTreeMap::new();
        let mut missing = Vec::new();
        for name in parsed.required_env() {
            match lookup(&name)
                .filter(|v| !v.is_empty())
                .or_else(|| dotenv.get(&name).cloned())
            {
                Some(value) => {
                    values.insert(name, Secret(value));
                }
                None => missing.push(name),
            }
        }
        if missing.is_empty() {
            Ok(Self { values })
        } else {
            Err(Error::MissingKeys { names: missing })
        }
    }

    /// The value of one variable.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Secret> {
        self.values.get(name)
    }

    /// Every resolved value, for scrubbing text before it is stored.
    pub(crate) fn values(&self) -> impl Iterator<Item = &str> {
        self.values.values().map(Secret::expose)
    }
}

/// Parse `KEY=value` lines; `#` comments, blank lines and `export ` prefixes
/// are allowed, and one pair of surrounding quotes is removed.
fn read_dotenv(path: &Path) -> Result<BTreeMap<String, String>> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(e) => return Err(Error::io("read", path, e)),
    };
    Ok(parse_dotenv(&text))
}

fn parse_dotenv(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            let line = line.strip_prefix("export ").unwrap_or(line);
            if line.starts_with('#') {
                return None;
            }
            let (key, value) = line.split_once('=')?;
            let value = value.trim();
            let value = [('"', '"'), ('\'', '\'')]
                .iter()
                .find_map(|(a, b)| value.strip_prefix(*a)?.strip_suffix(*b))
                .unwrap_or(value);
            Some((key.trim().to_owned(), value.to_owned()))
        })
        .collect()
}

#[cfg(test)]
mod test;
