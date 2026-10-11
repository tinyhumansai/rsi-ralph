//! Observe genuine pinned Lake checks on a host-private finite mathematics slice.
#[path = "verify_lean_slice/mod.rs"]
mod slice;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    slice::cli(&std::env::args().collect::<Vec<_>>())
}
