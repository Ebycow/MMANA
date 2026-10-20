//! MININEC3 method-of-moments engine (port of `mininec3.cpp`).
//!
//! The original is a line-by-line C translation of the MININEC3 BASIC
//! program and communicates through ~200 global variables. This port keeps
//! the numerical procedure **operation for operation** (same evaluation order,
//! same `f32` storage of the impedance matrix) so results match the original
//! to the last bits, while moving state into [`Mininec`] and passing the
//! values each routine actually uses as arguments.
//!
//! Indexing stays 1-based internally (index 0 unused) because pulse and wire
//! numbers are part of the algorithm (e.g. breakpoint index `2*w - 1 + p`).
//! The BASIC subroutine names (S196, L135, ...) are kept in comments so the
//! code can be read side by side with the original.

// Kept close to the reference code on purpose: 1-based index loops, NaN-aware
// `!(a > b)` tests and `x = x + y` statements mirror the original line by line.
#![allow(
    clippy::assign_op_pattern,
    clippy::needless_range_loop,
    clippy::neg_cmp_op_on_partial_ord,
    clippy::too_many_arguments,
    clippy::collapsible_if,
    clippy::implicit_saturating_sub
)]

pub(crate) mod antdef;
pub(crate) mod circuit;
mod currents;
mod farfield;
mod geometry;
mod kernel;
mod matrix;
mod nearfield;
mod pulse;
mod segment;

pub(crate) use pulse::to_half_width;

pub use farfield::FarFieldSample;
pub use nearfield::{AxisRange, NearFieldKind, NearFieldSample, NearFieldSpec};

/// Expanded wires (`WZMAX`).
pub const WZMAX: usize = 4096;
/// Highest S-domain order (`SMAX`).
pub const SMAX: usize = 12;
/// Combined-wire definitions (`PTMAX`) and sections per definition (`PPMAX`).
pub const PTMAX: usize = 10;
pub const PPMAX: usize = 10;
/// Largest pulse count the original can allocate (`PMAX`).
pub const PMAX: usize = 8192;

/// MMANA's "no value" marker.
pub(crate) const NULLV: f64 = 1.0e-99;
pub(crate) const PAI: f64 = std::f64::consts::PI;
pub(crate) const PAI2: f64 = PAI * 2.0;

#[inline]
pub(crate) fn sgn(c: i32) -> i32 {
    c.signum()
}

/// `INT()` of the BASIC original: floor for positive, (c-1) truncated for negative.
#[inline]
pub(crate) fn int_(c: f64) -> usize {
    debug_assert!(c >= 0.0);
    c as usize
}

/// One medium of a real ground as the engine sees it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MediumDef {
    pub rel: f64,
    pub cond: f64,
    pub boundary: f64,
    pub height: f64,
}

/// Environment values `InitAntData` reads from MMANA's global `env`.
#[derive(Debug, Clone)]
pub(crate) struct EngineConfig {
    pub ground: crate::model::GroundKind,
    pub height: f64,
    pub media: Vec<MediumDef>,
    pub screen: Option<(i32, f64)>,
    pub wire_material: crate::model::WireMaterial,
    pub swr_ro: f64,
    pub swr_jxo: f64,
    pub max_pulses: usize,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SourceDef {
    /// Pulse number (`aEi`).
    pub pulse: usize,
    /// Real / imaginary voltage (`aL`, `aM`).
    pub vr: f64,
    pub vi: f64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct LoadDef {
    /// Pulse number (`aLP`); 0 when the name did not resolve.
    pub pulse: usize,
    /// S-domain order (`aLS`), 0 for a plain impedance.
    pub ls: i32,
    /// `aLA[1..=2][..]`: numerator / denominator coefficients, or R / X at index 1.
    pub la: [[f64; SMAX + 1]; 3],
}

/// Feed-point data computed in S196 for each source.
#[derive(Debug, Clone, Copy)]
pub struct SourceSolution {
    pub pulse: usize,
    pub voltage: (f64, f64),
    pub current: (f64, f64),
    pub impedance: (f64, f64),
    pub power: f64,
    pub swr: f64,
}

/// Solver state for one geometry at one frequency.
pub(crate) struct Mininec {
    // constants
    pub p: f64,
    pub p0: f64,
    pub g0: f64,
    pub q: [f64; 15],
    pub c: [f64; 10],
    // frequency dependent
    pub f: f64,
    pub wavelength: f64,
    /// Wave number 2π/λ (`W`).
    pub w: f64,
    pub w2: f64,
    pub s0: f64,
    pub m: f64,
    pub srm: f64,
    // environment
    /// +1 free space, -1 ground plane (`G`).
    pub g: i32,
    pub nm: usize,
    pub tb: i32,
    pub nr: i32,
    pub rr: f64,
    pub med_t: Vec<f64>,
    pub med_v: Vec<f64>,
    pub med_u: Vec<f64>,
    pub med_h: Vec<f64>,
    pub height_off: f64,
    // wires, 1-based
    pub nw: usize,
    pub a: Vec<f64>,
    pub ca: Vec<f64>,
    pub cb: Vec<f64>,
    pub cg: Vec<f64>,
    pub s: Vec<f64>,
    pub j1: Vec<i32>,
    pub j2: Vec<[i32; 3]>,
    pub nn: Vec<[i32; 3]>,
    // pulses, 1-based
    pub n: usize,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub z: Vec<f64>,
    pub ci: Vec<[i32; 3]>,
    pub wi: Vec<i32>,
    // excitation and loads
    pub sources: Vec<SourceDef>,
    pub loads: Vec<LoadDef>,
    pub swr_ro: f64,
    pub swr_jxo: f64,
    // solution
    pub zr: Vec<f32>,
    pub zi: Vec<f32>,
    pub cr: Vec<f64>,
    pub cim: Vec<f64>,
    pub pwr: f64,
    pub solutions: Vec<SourceSolution>,
}

impl Mininec {
    /// Index into the N×N impedance matrix for 1-based (row, column).
    #[inline]
    pub(crate) fn mi(&self, row: usize, col: usize) -> usize {
        (row - 1) * self.n + (col - 1)
    }

    /// Segment break point `aX/aY/aZ[seg]` relative to the ground (`GetSegPos`).
    pub fn seg_pos(&self, seg: usize) -> (f64, f64, f64) {
        (self.x[seg], self.y[seg], self.z[seg] - self.height_off)
    }
}
