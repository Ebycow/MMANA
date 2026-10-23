//! One experiment's table, in Markdown.

use crate::experiments::{Case, Experiment};
use crate::metrics::{Grid, Metrics, measure, percent, refined};
use anyhow::Result;
use std::fmt::Write;

/// The figures of a case, of it with twice the segments, and of its reference.
struct Row {
    label: String,
    figures: Result<(Metrics, Metrics, Option<Metrics>)>,
}

fn row(case: Case, grid: Grid) -> Row {
    let figures = (|| {
        let m = measure(&case.project, grid)?;
        let doubled = measure(&refined(&case.project, 2), grid)?;
        let reference = case.reference.as_ref().map(|r| measure(r, grid)).transpose()?;
        Ok((m, doubled, reference))
    })();
    Row { label: case.label, figures }
}

fn signed(v: Option<f64>, digits: usize, unit: &str) -> String {
    v.map_or("—".into(), |v| format!("{v:+.digits$}{unit}"))
}

fn shift(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    Some(percent(a?, b?))
}

pub fn run(e: &Experiment, quick: bool, grid: Grid) -> Result<String> {
    let mut out = String::new();
    writeln!(out, "# {}\n", e.title)?;
    writeln!(out, "{}\n", e.description)?;
    writeln!(
        out,
        "`cargo run --release -p mmana-measure -- run {} --out docs/measurements` で生成（手で編集しない）。列の意味は [README](README.md#表の見方)。\n",
        e.id
    )?;
    writeln!(
        out,
        "| {} | 共振 (MHz) | 基準との差 | 分割 2 倍での変化 | R (Ω) | 分割 2 倍での R の変化 | 最大利得 (dBi) | 分割 2 倍での利得の変化 | AGT (dB) |",
        e.parameter
    )?;
    writeln!(out, "|---|--:|--:|--:|--:|--:|--:|--:|--:|")?;
    for case in (e.cases)(quick) {
        let r = row(case, grid);
        match r.figures {
            // a model MININEC cannot calculate: the input power is not positive
            Ok((m, _, _)) if m.r_ohm <= 0.0 => writeln!(
                out,
                "| {} | {} | | | {:.2} | | | | 破綻（R ≤ 0） |",
                r.label,
                m.resonance_mhz.map_or("なし".into(), |f| format!("{f:.4}")),
                m.r_ohm
            )?,
            Ok((m, d, reference)) => writeln!(
                out,
                "| {} | {} | {} | {} | {:.2} | {} | {:.2} | {} | {} |",
                r.label,
                m.resonance_mhz.map_or("なし".into(), |f| format!("{f:.4}")),
                signed(shift(m.resonance_mhz, reference.and_then(|r| r.resonance_mhz)), 3, " %"),
                signed(shift(d.resonance_mhz, m.resonance_mhz), 3, " %"),
                m.r_ohm,
                signed(Some(d.r_ohm - m.r_ohm), 2, " Ω"),
                m.gain_dbi,
                signed(Some(d.gain_dbi - m.gain_dbi), 2, " dB"),
                signed(m.agt_db, 2, ""),
            )?,
            Err(err) => writeln!(out, "| {} | 計算できない：{err} | | | | | | | |", r.label)?,
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::experiments::EXPERIMENTS;

    #[test]
    fn every_experiment_runs() {
        for e in EXPERIMENTS {
            let table = run(e, true, Grid::COARSE).unwrap();
            assert!(!table.contains("計算できない"), "{}:\n{table}", e.id);
            // a header, a rule and at least two cases
            assert!(table.lines().filter(|l| l.starts_with('|')).count() >= 4, "{}:\n{table}", e.id);
        }
    }
}
