//! Run or inspect complete source-backed Catalog recipes.
//! cargo run --features native --example samples -- table-square
//! cargo run --features native --example samples -- --source table-square
#[allow(dead_code)]
#[path = "catalog/samples/mod.rs"]
mod samples;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let name = args.next().unwrap_or_default();
    if name.is_empty() || matches!(name.as_str(), "--list" | "--help") {
        println!("Usage: samples <name> | --source <name> | --list");
        for (id, _) in samples::ALL {
            println!("  {}", id.replace('_', "-"));
        }
        return Ok(());
    }
    if name == "--source" {
        let name = args
            .next()
            .ok_or("--source needs a sample name")?
            .replace('-', "_");
        if !samples::ALL.iter().any(|(id, _)| *id == name) {
            return Err(format!("unknown sample {name}").into());
        }
        print!("{}", samples::code(&name));
        return Ok(());
    }
    samples::run(&name).ok_or_else(|| format!("unknown sample {name}; use --list"))?
}
