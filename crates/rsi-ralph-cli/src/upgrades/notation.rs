//! The short text forms `rsi-ralph upgrades add` and `outcome` accept on the
//! command line, each parsed into its record type:
//!
//! | Form | Example | Parses to |
//! | --- | --- | --- |
//! | `KIND:NAME[@MODEL][=ROLE]` | `assistant:claude-code@claude-opus-5-5=diagnosed` | [`Contributor`] |
//! | `METRIC=VALUE[@RUN]` | `seats_busy_mean=7.2@erdos-779/20261025-103058` | [`Evidence`] |
//! | `METRIC OP VALUE` | `seats_busy_mean>=10` | [`Prediction`] |
//! | `PATH[#KEY,KEY]` | `hive/constitution.json#max_turns` | [`FileChange`] |
//!
//! `KIND` is `operator`, `assistant`, `hive` (the name is the seat) or
//! `loop` (the name is the mechanism); only an assistant takes `@MODEL`.
//! `OP` is one of `>=`, `<=`, `>`, `<`, `=`.

use super::{Author, Comparison, Contributor, Evidence, FileChange, Prediction, Role, Verdict};
use crate::{Error, Result};

/// Comparison operators, the two-character ones first.
const OPS: [(&str, Comparison); 5] = [
    (">=", Comparison::Ge),
    ("<=", Comparison::Le),
    (">", Comparison::Gt),
    ("<", Comparison::Lt),
    ("=", Comparison::Eq),
];

/// Parse a contributor, `KIND:NAME[@MODEL][=ROLE]`; the role defaults to
/// `authored`.
///
/// # Errors
/// Returns [`Error::Field`] naming `--author` for an unknown kind or role, a
/// missing name, or a model on anyone but an assistant.
pub fn contributor(text: &str) -> Result<Contributor> {
    let bad = |reason: &str| Error::field(format!("--author {text}"), reason);
    let (kind, rest) = text
        .split_once(':')
        .ok_or_else(|| bad("expected KIND:NAME, such as operator:steven"))?;
    let (rest, role) = match rest.rsplit_once('=') {
        Some((rest, role)) => (rest, parse_role(role).ok_or_else(|| bad("unknown role"))?),
        None => (rest, Role::Authored),
    };
    let (name, model) = match rest.split_once('@') {
        Some((name, model)) if !model.is_empty() => (name, Some(model.to_owned())),
        Some(_) => return Err(bad("empty model")),
        None => (rest, None),
    };
    if name.is_empty() {
        return Err(bad("empty name"));
    }
    let name = name.to_owned();
    let author = match (kind, model) {
        ("assistant", model) => Author::Assistant { name, model },
        (_, Some(_)) => return Err(bad("only an assistant names a model")),
        ("operator", None) => Author::Operator { name },
        ("hive", None) => Author::Hive { seat: name },
        ("loop", None) => Author::Loop { mechanism: name },
        _ => return Err(bad("kind must be operator, assistant, hive or loop")),
    };
    Ok(Contributor {
        author,
        role,
        rationale: None,
    })
}

fn parse_role(text: &str) -> Option<Role> {
    serde_json::from_value(serde_json::Value::String(text.to_owned())).ok()
}

/// Parse a verdict: `confirmed`, `refuted` or `mixed`.
///
/// # Errors
/// Returns [`Error::Field`] naming `--verdict` for anything else.
pub fn verdict(text: &str) -> Result<Verdict> {
    serde_json::from_value(serde_json::Value::String(text.to_owned()))
        .map_err(|_| Error::field("--verdict", "expected confirmed, refuted or mixed"))
}

/// Parse a measured value, `METRIC=VALUE[@RUN]`.
///
/// # Errors
/// Returns [`Error::Field`] naming `--metric` for a missing name or a value
/// that is not a finite number.
pub fn metric(text: &str) -> Result<Evidence> {
    let bad = |reason: &str| Error::field(format!("--metric {text}"), reason);
    let (name, rest) = text
        .split_once('=')
        .filter(|(name, _)| !name.is_empty())
        .ok_or_else(|| bad("expected METRIC=VALUE[@RUN]"))?;
    let (value, run) = match rest.split_once('@') {
        Some((value, run)) if !run.is_empty() => (value, Some(run.to_owned())),
        Some(_) => return Err(bad("empty run")),
        None => (rest, None),
    };
    let value: f64 = value
        .trim()
        .parse()
        .ok()
        .filter(|v: &f64| v.is_finite())
        .ok_or_else(|| bad("value must be a finite number"))?;
    Ok(Evidence {
        metric: Some(name.trim().to_owned()),
        value: Some(value),
        run,
        ..Evidence::default()
    })
}

/// Parse a prediction, `METRIC OP VALUE` with `OP` one of `>=`, `<=`, `>`,
/// `<`, `=`.
///
/// # Errors
/// Returns [`Error::Field`] naming `--expect` for a missing operator, name
/// or finite value.
pub fn prediction(text: &str) -> Result<Prediction> {
    let bad = |reason: &str| Error::field(format!("--expect {text}"), reason);
    let (at, symbol, op) = OPS
        .iter()
        .filter_map(|(symbol, op)| text.find(symbol).map(|at| (at, *symbol, *op)))
        .min_by_key(|(at, symbol, _)| (*at, std::cmp::Reverse(symbol.len())))
        .ok_or_else(|| bad("expected METRIC OP VALUE, such as seats_busy_mean>=10"))?;
    let metric = text[..at].trim();
    let value: f64 = text[at + symbol.len()..]
        .trim()
        .parse()
        .ok()
        .filter(|v: &f64| v.is_finite())
        .ok_or_else(|| bad("value must be a finite number"))?;
    if metric.is_empty() {
        return Err(bad("empty metric"));
    }
    Ok(Prediction {
        metric: metric.to_owned(),
        op,
        value,
        note: None,
    })
}

/// Parse a touched file, `PATH[#KEY,KEY]`. Hashes are filled in by the
/// caller.
///
/// # Errors
/// Returns [`Error::Field`] naming `--file` for an empty, absolute or
/// escaping path.
pub fn file(text: &str) -> Result<FileChange> {
    let (path, keys) = match text.split_once('#') {
        Some((path, keys)) => (
            path,
            keys.split(',')
                .map(str::trim)
                .filter(|k| !k.is_empty())
                .map(str::to_owned)
                .collect(),
        ),
        None => (text, Vec::new()),
    };
    let path = path.trim_start_matches("./");
    if path.is_empty() || path.starts_with('/') || path.split('/').any(|part| part == "..") {
        return Err(Error::field(
            format!("--file {text}"),
            "expected a path inside the loop folder",
        ));
    }
    Ok(FileChange {
        path: path.to_owned(),
        keys,
        before: None,
        after: None,
    })
}
