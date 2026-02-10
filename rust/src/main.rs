use anyhow::Result;
use clap::Parser;
use std::io::{self, Write};

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

    let progress_step_percent = 5usize;
    let mut last_pass = 0usize;
    let mut last_bucket: isize = -1;
    let out = engine.run_with_progress(|p| {
        if p.current_pass != last_pass && last_pass != 0 {
            eprintln!();
        }
        if p.current_pass != last_pass {
            last_bucket = -1;
        }
        last_pass = p.current_pass;

        let percent = if p.total_features == 0 {
            100.0
        } else {
            (p.current_feature as f64 * 100.0) / p.total_features as f64
        };

        let bucket = (percent / progress_step_percent as f64).floor() as isize;
        let should_print = p.current_feature == 1
            || p.current_feature == p.total_features
            || bucket != last_bucket;

        if !should_print {
            return;
        }
        last_bucket = bucket;

        eprint!(
            "\rPass {}: {}/{} ({percent:.1}%)",
            p.current_pass, p.current_feature, p.total_features
        );
        let _ = io::stderr().flush();
    })?;
    if last_pass > 0 {
        eprintln!();
    }

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
