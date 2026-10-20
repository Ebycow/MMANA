//! `mmana` command-line tool.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use mmana_core::{CalcOptions, PatternResolution, Project, maa};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "mmana", version, about = "MININEC3 antenna analysis (MMANA port)")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Calculate an antenna (.maa or project .json).
    Calc {
        file: PathBuf,
        /// Calculation frequency in MHz (default: design frequency).
        #[arg(long)]
        freq: Option<f64>,
        /// Use 2° pattern resolution.
        #[arg(long)]
        coarse: bool,
        /// Print the full result as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Frequency sweep.
    Sweep {
        file: PathBuf,
        #[arg(long)]
        start: f64,
        #[arg(long)]
        stop: f64,
        #[arg(long, default_value_t = 11)]
        points: usize,
        #[arg(long)]
        json: bool,
    },
    /// Convert a .maa file to project JSON.
    ToJson { file: PathBuf },
    /// Convert project JSON to a .maa file (Shift_JIS).
    ToMaa {
        file: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
}

fn load(path: &Path) -> Result<Project> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("json")) {
        return Ok(serde_json::from_slice(&bytes)?);
    }
    let parsed = maa::parse_bytes(&bytes);
    for w in &parsed.warnings {
        eprintln!("warning: {w}");
    }
    Ok(parsed.project)
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Calc { file, freq, coarse, json } => {
            let project = load(&file)?;
            let opts = CalcOptions {
                frequency_mhz: freq,
                pattern_resolution: if coarse { PatternResolution::Coarse } else { PatternResolution::Fine },
                ..Default::default()
            };
            let r = mmana_core::calculate(&project, &opts)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&r)?);
                return Ok(());
            }
            println!("{}", project.antenna.name);
            println!("  frequency   {:.3} MHz   ({} pulses)", r.frequency_mhz, r.pulse_count);
            println!("  impedance   {:.2} {:+.2}j Ω", r.impedance.re, r.impedance.im);
            println!("  SWR         {:.2}  ({} Ω ref)", r.swr, project.environment.swr_reference.r);
            println!("  gain        {:.2} dBi", r.gain_dbi);
            println!("  F/B         {:.2} dB", r.fb_db);
            println!("  elevation   {:.1}°", r.elevation_deg);
            for w in &r.warnings {
                println!("  warning: {w}");
            }
        }
        Command::Sweep { file, start, stop, points, json } => {
            let project = load(&file)?;
            let pts = mmana_core::sweep(&project, &CalcOptions::default(), start, stop, points)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&pts)?);
                return Ok(());
            }
            println!("{:>10} {:>9} {:>9} {:>8} {:>8} {:>7}", "MHz", "R", "jX", "SWR", "dBi", "F/B");
            for p in pts {
                println!(
                    "{:>10.4} {:>9.2} {:>9.2} {:>8.2} {:>8.2} {:>7.2}",
                    p.frequency_mhz, p.impedance.re, p.impedance.im, p.swr, p.gain_dbi, p.fb_db
                );
            }
        }
        Command::ToJson { file } => println!("{}", serde_json::to_string_pretty(&load(&file)?)?),
        Command::ToMaa { file, output } => {
            let text = maa::write(&load(&file)?);
            std::fs::write(&output, maa::encode(&text)).with_context(|| format!("writing {}", output.display()))?;
        }
    }
    Ok(())
}
