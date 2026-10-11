//! Load a private agent-authored bank and print only frozen suite fingerprints.
#[path = "private_benchmark/mod.rs"]
mod bank;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    bank::cli(&std::env::args().collect::<Vec<_>>())
}
