//! `mmana` command-line tool.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use mmana_core::optimize::{self, OptimizerSetup, StopReason};
use mmana_core::resonance::{self, ResonanceRange};
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
    /// Find the resonant frequency nearest to a start frequency (X = 0, rising).
    Resonance {
        file: PathBuf,
        /// Start frequency in MHz (default: design frequency).
        #[arg(long)]
        start: Option<f64>,
        /// Lowest frequency searched (default: half the start).
        #[arg(long)]
        min: Option<f64>,
        /// Highest frequency searched (default: twice the start).
        #[arg(long)]
        max: Option<f64>,
        #[arg(long)]
        json: bool,
    },
    /// Optimise an antenna with a setup (JSON `OptimizerSetup`, as the API takes it).
    Optimize {
        file: PathBuf,
        #[arg(long)]
        setup: PathBuf,
        /// Starting calculation frequency in MHz (default: design frequency).
        #[arg(long)]
        freq: Option<f64>,
        /// Stop after this many calculations.
        #[arg(long)]
        limit: Option<usize>,
        /// Write the optimised antenna (.maa, or project JSON by extension).
        #[arg(short, long)]
        output: Option<PathBuf>,
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
        Command::Resonance { file, start, min, max, json } => {
            let project = load(&file)?;
            let range = ResonanceRange { start_mhz: start, min_mhz: min, max_mhz: max };
            let found = resonance::find_resonance(&project, &CalcOptions::default(), &range)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&found)?);
                return Ok(());
            }
            println!("{}", project.antenna.name);
            let Some(r) = found.resonance else {
                println!(
                    "  no resonance from {:.4} to {:.4} MHz ({} calculations)",
                    found.min_mhz, found.max_mhz, found.calculations
                );
                return Ok(());
            };
            println!("  resonance   {:.4} MHz   ({} calculations)", r.frequency_mhz, found.calculations);
            println!("  impedance   {:.2} {:+.2}j Ω", r.impedance.re, r.impedance.im);
            println!("  SWR         {:.2}  ({} Ω ref)", r.swr, project.environment.swr_reference.r);
            if let Some(c) = r.circuit {
                println!(
                    "  series      L {:.4} µH   C {:.4} pF   Q {:.2}   bandwidth {:.1} kHz",
                    c.inductance_uh, c.capacitance_pf, c.q, c.bandwidth_khz
                );
            }
            if !r.converged {
                println!("  warning: the reactance jumps across 0 here (the segmentation changes with the frequency)");
            }
        }
        Command::Optimize { file, setup, freq, limit, output } => {
            let project = load(&file)?;
            let bytes = std::fs::read(&setup).with_context(|| format!("reading {}", setup.display()))?;
            let setup: OptimizerSetup = serde_json::from_slice(&bytes)?;
            let opts = CalcOptions { frequency_mhz: freq, ..Default::default() };
            println!(
                "{:>6} {:>4} {:>12} {:>8} {:>8} {:>7} {:>7} {:>6} {:>5}",
                "calc", "var", "value", "R", "jX", "SWR", "dBi", "F/B", "elev"
            );
            let out = optimize::optimize(&project, &opts, &setup, limit, |p| {
                println!(
                    "{:>6} {:>4} {:>12.6} {:>8.2} {:>8.2} {:>7.2} {:>7.2} {:>6.2} {:>5.1}{}",
                    p.calculation,
                    p.variable + 1,
                    p.values[p.variable],
                    p.impedance.re,
                    p.impedance.im,
                    p.swr,
                    p.gain_dbi,
                    p.fb_db,
                    p.elevation_deg,
                    if p.improved { " *" } else { "" }
                );
                true
            })?;
            for w in &out.warnings {
                eprintln!("warning: {}", w.message);
            }
            match &out.stop {
                StopReason::Converged => println!("converged after {} calculations", out.calculations),
                StopReason::CalculationLimit => println!("stopped at the limit of {} calculations", out.calculations),
                StopReason::Cancelled => println!("cancelled"),
                StopReason::Failed { message } => println!("stopped: {message}"),
            }
            if let Some(best) = &out.best {
                let values: Vec<String> = best.values.iter().map(|v| format!("{v}")).collect();
                println!("best values: {}", values.join(", "));
            }
            if let Some(output) = output {
                let bytes = if output.extension().is_some_and(|e| e.eq_ignore_ascii_case("json")) {
                    serde_json::to_vec_pretty(&out.project)?
                } else {
                    maa::encode(&maa::write(&out.project))
                };
                std::fs::write(&output, bytes).with_context(|| format!("writing {}", output.display()))?;
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
