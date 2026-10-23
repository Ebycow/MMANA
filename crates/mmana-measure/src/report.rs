//! One experiment's table, in Markdown, and whether its warnings match it.

use crate::experiments::{Case, EXPERIMENTS, Experiment};
use crate::metrics::{Grid, Metrics, measure, percent, refined};
use anyhow::Result;
use mmana_core::WarningCode;
use std::fmt::Write;

/// How far the resonance may move before a result is not to be trusted (%).
/// Builders trim the real antenna for errors of a few percent; the
/// calculation's own error should stay well under that.
pub const TOLERANCE: f64 = 0.5;

/// A case measured: as it is, with twice the segments, and its reference.
pub struct Measured {
    pub label: String,
    pub figures: Result<(Metrics, Metrics, Option<Metrics>)>,
}

fn shift(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    Some(percent(a?, b?))
}

impl Measured {
    fn new(case: Case, grid: Grid) -> Self {
        let figures = (|| {
            let m = measure(&case.project, grid)?;
            let doubled = measure(&refined(&case.project, 2), grid)?;
            let reference = case.reference.as_ref().map(|r| measure(r, grid)).transpose()?;
            Ok((m, doubled, reference))
        })();
        Measured { label: case.label, figures }
    }

    /// How far the resonance is off (%): from the reference when there is
    /// one, else from twice the segments. Infinite when MININEC broke down
    /// (no input power) or the resonance went missing.
    pub fn error(&self) -> Option<f64> {
        let (m, d, reference) = self.figures.as_ref().ok()?;
        if m.r_ohm <= 0.0 {
            return Some(f64::INFINITY);
        }
        let against = reference.as_ref().unwrap_or(d);
        match (m.resonance_mhz, against.resonance_mhz) {
            (None, None) => None,
            (Some(a), Some(b)) => Some(percent(a, b).abs()),
            _ => Some(f64::INFINITY),
        }
    }
}

pub fn measure_all(e: &Experiment, quick: bool, grid: Grid) -> Vec<Measured> {
    (e.cases)(quick).into_iter().map(|c| Measured::new(c, grid)).collect()
}

fn signed(v: Option<f64>, digits: usize, unit: &str) -> String {
    v.map_or("—".into(), |v| format!("{v:+.digits$}{unit}"))
}

/// The warnings these experiments decide on: those some experiment is meant to
/// raise. (The others rest on measurements of their own, such as the drift
/// of the reactance with many segments.)
fn governed() -> Vec<WarningCode> {
    EXPERIMENTS.iter().flat_map(|e| e.catches.iter().copied()).collect()
}

/// Where the warnings disagree with the measurement: a case off by more than
/// [`TOLERANCE`] without a warning the experiment is meant to raise, or one of
/// the [`governed`] warnings on a case within half of it.
pub fn mismatches(e: &Experiment, rows: &[Measured]) -> Vec<String> {
    let mut out = Vec::new();
    for r in rows {
        let (Ok((m, _, _)), Some(error)) = (&r.figures, r.error()) else { continue };
        let warned: Vec<_> = m.warnings.iter().filter(|c| e.catches.contains(c)).collect();
        if error > TOLERANCE && !e.catches.is_empty() && warned.is_empty() {
            out.push(format!("{}: {}: off by {error:.3} % without a warning", e.id, r.label));
        }
        let alarms: Vec<_> = m.warnings.iter().filter(|c| governed().contains(c)).collect();
        if error < TOLERANCE / 2.0 && !alarms.is_empty() {
            out.push(format!("{}: {}: warned {alarms:?} though within {error:.3} %", e.id, r.label));
        }
    }
    out
}

pub fn table(e: &Experiment, rows: &[Measured]) -> Result<String> {
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
        "| {} | 共振 (MHz) | 基準との差 | 分割 2 倍での変化 | R (Ω) | 分割 2 倍での R の変化 | 最大利得 (dBi) | 分割 2 倍での利得の変化 | AGT (dB) | 警告 |",
        e.parameter
    )?;
    writeln!(out, "|---|--:|--:|--:|--:|--:|--:|--:|--:|---|")?;
    for r in rows {
        let resonance = |m: &Metrics| m.resonance_mhz.map_or("なし".into(), |f| format!("{f:.4}"));
        match &r.figures {
            Ok((m, d, reference)) => {
                let warnings = m.warnings.iter().map(|c| format!("`{c:?}`")).collect::<Vec<_>>().join(" ");
                if m.r_ohm <= 0.0 {
                    // MININEC cannot calculate it: the input power is not positive
                    writeln!(
                        out,
                        "| {} | {} | | | {:.2} | | | | 破綻（R ≤ 0） | {warnings} |",
                        r.label,
                        resonance(m),
                        m.r_ohm
                    )?;
                    continue;
                }
                writeln!(
                    out,
                    "| {} | {} | {} | {} | {:.2} | {} | {:.2} | {} | {} | {warnings} |",
                    r.label,
                    resonance(m),
                    signed(shift(m.resonance_mhz, reference.as_ref().and_then(|r| r.resonance_mhz)), 3, " %"),
                    signed(shift(d.resonance_mhz, m.resonance_mhz), 3, " %"),
                    m.r_ohm,
                    signed(Some(d.r_ohm - m.r_ohm), 2, " Ω"),
                    m.gain_dbi,
                    signed(Some(d.gain_dbi - m.gain_dbi), 2, " dB"),
                    signed(m.agt_db, 2, ""),
                )?;
            }
            Err(err) => writeln!(out, "| {} | 計算できない：{err} | | | | | | | | |", r.label)?,
        }
    }
    let wrong = mismatches(e, rows);
    if !e.catches.is_empty() || !wrong.is_empty() {
        let codes = e.catches.iter().map(|c| format!("`{c:?}`")).collect::<Vec<_>>().join("・");
        writeln!(out, "\n## 警告と測定の一致\n")?;
        if !codes.is_empty() {
            writeln!(
                out,
                "この実験で出るべき警告：{codes}。共振のずれ（基準との差、なければ分割 2 倍での変化）が {TOLERANCE} % を超えるケースに出ること、{} % 未満のケースには出ないことを確かめる。\n",
                TOLERANCE / 2.0
            )?;
        }
        if wrong.is_empty() {
            writeln!(out, "すべて一致。")?;
        } else {
            for w in &wrong {
                writeln!(out, "- 不一致：{w}")?;
            }
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
            let rows = measure_all(e, true, Grid::COARSE);
            let t = table(e, &rows).unwrap();
            assert!(!t.contains("計算できない"), "{}:\n{t}", e.id);
            // a header, a rule and at least two cases
            assert!(t.lines().filter(|l| l.starts_with('|')).count() >= 4, "{}:\n{t}", e.id);
        }
    }

    /// The warnings appear where the results move and nowhere else, over
    /// every case of every experiment.
    #[test]
    fn warnings_match_the_measurements() {
        let wrong: Vec<_> =
            EXPERIMENTS.iter().flat_map(|e| mismatches(e, &measure_all(e, false, Grid::COARSE))).collect();
        assert!(wrong.is_empty(), "{wrong:#?}");
    }
}
