//! Run any Catalog code sample as its own app:
//!
//! cargo run --features native --example samples -- button-variants
//! cargo run --features native --example samples            # list them
#[allow(dead_code)]
#[path = "catalog/samples/mod.rs"]
mod samples;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let name = std::env::args().nth(1).unwrap_or_default();
    match samples::run(&name) {
        Some(result) => result,
        None => {
            eprintln!("Usage: cargo run --features native --example samples -- <name>\n");
            for (sample, _) in samples::ALL {
                eprintln!("  {}", sample.replace('_', "-"));
            }
            std::process::exit(if name.is_empty() { 0 } else { 2 });
        }
    }
}
