//! Project a quiescent private journal/archive into a safe selection-only report.
use std::{env, error::Error};
use tinyrsi_host::{Archive, EpochJournal};
/// Reads persisted host evidence; no provider call or evaluation is repeated.
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.len() != 4 {
        return Err("usage: proposal_metrics <journal> <archive> <epoch> <k>".into());
    }
    let journal = EpochJournal::open(&args[0])?;
    let archive = Archive::open(&args[1])?;
    let dataset = journal
        .proposal_dataset(args[2].parse()?, &archive)?
        .ok_or("epoch has no known generation identity")?;
    let k = args[3].parse()?;
    let metrics = if dataset.rows.is_empty() {
        None
    } else {
        Some(dataset.metrics(k)?)
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({"dataset": dataset, "metrics": metrics}))?
    );
    Ok(())
}
