use anyhow::Result;
use clap::Parser;

use dp_simplify::io::{read_gpkg, write_gpkg};
use dp_simplify::{SimplifyEngine, SimplifyParams};

#[derive(Debug, Parser)]
#[command(name = "dp_simplify")]
#[command(about = "Topology-aware Douglas-Peucker simplification for GeoPackage layers")]
struct Cli {
    #[arg(long)]
    input: String,

    #[arg(long)]
    output: String,

    #[arg(long)]
    tolerance: f64,

    #[arg(long)]
    layer: Option<String>,

    #[arg(long, default_value_t = false)]
    validate_structure: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let (records, schema) = read_gpkg(&cli.input, cli.layer.as_deref())?;

    let engine = SimplifyEngine::new(
        &records,
        SimplifyParams {
            tolerance: cli.tolerance,
            validate_structure: cli.validate_structure,
        },
    )?;

    let out = engine.run()?;
    write_gpkg(&cli.output, &out.features, &schema)?;

    println!(
        "Simplified: in_features={} out_features={} deleted_vertices={} passes={}",
        out.stats.in_nbr_features,
        out.stats.out_nbr_features,
        out.stats.nbr_vertice_deleted,
        out.stats.nbr_pass
    );

    if let Some(valid) = out.stats.is_structure_valid {
        println!("Structure valid: {valid}");
    }

    Ok(())
}
