//! Engine benchmark: median time of `calculate()` on samples of increasing
//! size, plus a 21-point `sweep()` (what the desktop app runs by default).
//!
//! ```sh
//! cargo run --release -p mmana-core --example bench            # default samples
//! cargo run --release -p mmana-core --example bench -- CR2 4X12 # selected samples
//! cargo run --release -p mmana-core --example bench -- --calc-only CR2
//! ```
//!
//! `RAYON_NUM_THREADS=1` measures the engine on one thread.
//!
//! Compare with the original C++ engine using `reference/oracle/bench.sh`.

use mmana_core::{CalcOptions, Project, calculate, maa, sweep};
use std::path::Path;
use std::time::Instant;

const DEFAULT: &[&str] =
    &["VERT20", "2EL20", "GP40", "TWIND", "COLLINEA", "15EL23CM", "3HENT", "4X12", "12EL23CM", "CR2"];

fn load(stem: &str) -> Project {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../testdata/maa/{stem}.MAA"));
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    maa::parse_bytes(&bytes).project
}

/// Median wall time in ms over enough runs to fill about a second.
fn median_ms(mut f: impl FnMut()) -> f64 {
    let t0 = Instant::now();
    f(); // warm-up
    let once = t0.elapsed().as_secs_f64();
    let runs = (1.0 / once.max(1e-6)).clamp(5.0, 200.0) as usize;
    let mut t: Vec<f64> = (0..runs)
        .map(|_| {
            let t0 = Instant::now();
            f();
            t0.elapsed().as_secs_f64() * 1e3
        })
        .collect();
    t.sort_by(f64::total_cmp);
    t[t.len() / 2]
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let calc_only = args.iter().any(|a| a == "--calc-only");
    args.retain(|a| a != "--calc-only");
    let stems: Vec<&str> = if args.is_empty() { DEFAULT.to_vec() } else { args.iter().map(String::as_str).collect() };
    let opts = CalcOptions::default();
    println!("{:<10} {:>7} {:>12} {:>14}", "sample", "pulses", "calc [ms]", "sweep21 [ms]");
    for stem in stems {
        let project = load(stem);
        let n = calculate(&project, &opts).unwrap().pulse_count;
        let calc = median_ms(|| {
            std::hint::black_box(calculate(&project, &opts).unwrap());
        });
        let f = project.antenna.frequency_mhz;
        let sw = if calc_only {
            "-".to_owned()
        } else {
            let ms = median_ms(|| {
                std::hint::black_box(sweep(&project, &opts, f * 0.98, f * 1.02, 21).unwrap());
            });
            format!("{ms:.2}")
        };
        println!("{stem:<10} {n:>7} {calc:>12.3} {sw:>14}");
    }
}
