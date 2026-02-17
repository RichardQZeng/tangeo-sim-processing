use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use std::io::{self, Write};

use dp_simplify::io::{read_gpkg, write_gpkg};
use dp_simplify::{ReduceBendEngine, ReduceBendParams, SimplifyEngine, SimplifyParams};

#[derive(Debug, Parser)]
#[command(name = "dp_simplify")]
#[command(about = "Topology-aware Douglas-Peucker simplification for GeoPackage layers")]
struct Cli {
    #[arg(long)]
    input: Option<String>,

    #[arg(long)]
    output: Option<String>,

    #[arg(long)]
    tolerance: Option<f64>,

    #[arg(long)]
    layer: Option<String>,

    #[arg(long, default_value_t = false)]
    validate_structure: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    Simplify(SimplifyArgs),
    ReduceBend(ReduceBendArgs),
}

#[derive(Debug, Parser)]
struct SimplifyArgs {
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

#[derive(Debug, Parser)]
struct ReduceBendArgs {
    #[arg(long)]
    input: String,
    #[arg(long)]
    output: String,
    #[arg(long)]
    diameter: f64,
    #[arg(long)]
    layer: Option<String>,
    #[arg(long, default_value_t = false)]
    smooth_line: bool,
    #[arg(long, default_value_t = false)]
    del_outer: bool,
    #[arg(long, default_value_t = false)]
    del_inner: bool,
    #[arg(long, default_value_t = false)]
    validate_structure: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Simplify(args)) => run_simplify(args),
        Some(Command::ReduceBend(args)) => run_reduce_bend(args),
        None => {
            let input = cli
                .input
                .ok_or_else(|| anyhow!("legacy mode requires --input"))?;
            let output = cli
                .output
                .ok_or_else(|| anyhow!("legacy mode requires --output"))?;
            let tolerance = cli
                .tolerance
                .ok_or_else(|| anyhow!("legacy mode requires --tolerance"))?;

            run_simplify(SimplifyArgs {
                input,
                output,
                tolerance,
                layer: cli.layer,
                validate_structure: cli.validate_structure,
            })
        }
    }
}

fn run_simplify(args: SimplifyArgs) -> Result<()> {
    let (records, schema) = read_gpkg(&args.input, args.layer.as_deref())?;

    let engine = SimplifyEngine::new(
        &records,
        SimplifyParams {
            tolerance: args.tolerance,
            validate_structure: args.validate_structure,
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

    write_gpkg(&args.output, &out.features, &schema)?;

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

fn run_reduce_bend(args: ReduceBendArgs) -> Result<()> {
    let (records, schema) = read_gpkg(&args.input, args.layer.as_deref())?;
    let engine = ReduceBendEngine::new(
        &records,
        ReduceBendParams {
            diameter_tol: args.diameter,
            smooth_line: args.smooth_line,
            flag_del_outer: args.del_outer,
            flag_del_inner: args.del_inner,
            validate_structure: args.validate_structure,
        },
    )?;

    let out = engine.run()?;
    write_gpkg(&args.output, &out.features, &schema)?;

    println!(
        "Reduce-bend: in_features={} out_features={} reduced_bends={} detected_bends={} passes={} holes_deleted={} polygons_deleted={} smoothed_lines={}",
        out.stats.in_nbr_features,
        out.stats.out_nbr_features,
        out.stats.nbr_bend_reduced,
        out.stats.nbr_bend_detected,
        out.stats.nbr_pass,
        out.stats.nbr_hole_del,
        out.stats.nbr_pol_del,
        out.stats.nbr_line_smooth
    );
    if let Some(valid) = out.stats.is_structure_valid {
        println!("Structure valid: {valid}");
    }

    Ok(())
}
