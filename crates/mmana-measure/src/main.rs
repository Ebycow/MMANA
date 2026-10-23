//! `mmana-measure`: how much the results move when an antenna does what
//! MININEC may model badly (issue #16). Each experiment varies one thing over
//! a family of antennas and writes a table to `docs/measurements/<id>.md`;
//! which of them deserve a warning is decided from those tables.
//!
//!     cargo run --release -p mmana-measure -- list
//!     cargo run --release -p mmana-measure -- run segments v-angle
//!     cargo run --release -p mmana-measure -- run --all

mod experiments;
mod metrics;
mod report;

use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use experiments::EXPERIMENTS;
use metrics::Grid;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "mmana-measure",
    about = "Measures how far MININEC's results move when a model does what it handles badly"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List the experiments.
    List,
    /// Run experiments and write their tables.
    Run {
        /// Experiment ids (see `list`).
        ids: Vec<String>,
        #[arg(long)]
        all: bool,
        /// A few cases on a coarse grid, to check the experiments run.
        #[arg(long)]
        quick: bool,
        /// Where the tables go; printed instead when omitted.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Write the antennas of an experiment as .maa files, to open them in the app.
    Dump {
        id: String,
        #[arg(long)]
        out: PathBuf,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::List => {
            for e in EXPERIMENTS {
                println!("{:16} {}", e.id, e.title);
            }
        }
        Command::Dump { id, out } => {
            let e = EXPERIMENTS.iter().find(|e| e.id == id).ok_or_else(|| anyhow::anyhow!("no experiment {id}"))?;
            std::fs::create_dir_all(&out)?;
            for (i, case) in (e.cases)(false).into_iter().enumerate() {
                let path = out.join(format!("{}-{}.maa", e.id, i + 1));
                std::fs::write(&path, mmana_core::maa::encode(&mmana_core::maa::write(&case.project)))?;
                println!("{}  {}", path.display(), case.label);
            }
        }
        Command::Run { ids, all, quick, out } => {
            let chosen: Vec<_> = if all {
                EXPERIMENTS.iter().collect()
            } else {
                ids.iter()
                    .map(|id| {
                        EXPERIMENTS.iter().find(|e| e.id == id).ok_or_else(|| anyhow::anyhow!("no experiment {id}"))
                    })
                    .collect::<Result<_>>()?
            };
            if chosen.is_empty() {
                bail!("name experiments or pass --all");
            }
            let grid = if quick { Grid::COARSE } else { Grid::FINE };
            let mut wrong = Vec::new();
            for e in chosen {
                eprintln!("{} …", e.id);
                let rows = report::measure_all(e, quick, grid);
                wrong.extend(report::mismatches(e, &rows));
                let table = report::table(e, &rows)?;
                match &out {
                    Some(dir) => {
                        std::fs::create_dir_all(dir)?;
                        std::fs::write(dir.join(format!("{}.md", e.id)), table)?;
                    }
                    None => println!("{table}"),
                }
            }
            for w in &wrong {
                eprintln!("mismatch: {w}");
            }
            if !wrong.is_empty() {
                bail!("{} cases where the warnings disagree with the measurements", wrong.len());
            }
        }
    }
    Ok(())
}
