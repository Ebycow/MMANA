//! # mmana-core
//!
//! Antenna analysis engine ported from MMANA (MININEC3). Pure Rust, no I/O:
//! usable from the HTTP API, the desktop app (Tauri) and WebAssembly alike.
//!
//! ```
//! use mmana_core::{calculate, maa, CalcOptions};
//!
//! let text = "Dipole\n*\n14.05\n*** Wires ***\n1\n0, -5.3, 0, 0, 5.3, 0, 0.001, -1\n\
//!             *** Source ***\n1, 1\nw1c, 0, 1\n*** Load ***\n0, 1\n\
//!             *** Segmentation ***\n400, 40, 2.0, 1\n*** Env ***\n0, 20, 0, 50, 120, 60, 0\n";
//! let project = maa::parse(text).project;
//! let result = calculate(&project, &CalcOptions::default()).unwrap();
//! assert!(result.impedance.re > 60.0 && result.impedance.re < 80.0);
//! ```

mod calc;
pub mod contract;
pub mod elements;
pub mod engine;
mod error;
pub mod expr;
pub mod format;
pub mod maa;
pub mod model;
pub mod optimize;
mod par;
mod pattern;
mod validate;

pub use calc::{
    CalcOptions, CalcResult, CalcWarning, Complex, PatternCut, PatternResolution, Patterns, PulseCurrent, PulseLayout,
    PulsePoint, SegmentedWire, Solved, SourceResult, SweepPoint, WarningCode, WirePulses, calculate, pulse_layout,
    solve, sweep, sweep_at, sweep_frequency,
};
pub use engine::{AxisRange, FarFieldSample, NearFieldKind, NearFieldSample, NearFieldSpec};
pub use error::CalcError;
pub use model::*;
pub use validate::{MAX_FIELD_POINTS, MAX_SWEEP_POINTS};
