//! Optimisation (MMANA "最適化").
//!
//! Varies wire coordinates, element sizes, loads, the height, the frequency,
//! sources or the stacking distance one variable at a time, keeps the value
//! that scores best on a weighted mix of gain, F/B, jX, SWR, elevation, a
//! matching network and a pulse current, and repeats until a whole pass
//! improves nothing. Several bands can be scored together.
//!
//! Port of `CACal` (ComLib.cpp), its loop `CACal::Exec` and one trial
//! `TMainWnd::ACalExecute` (Main.cpp), and the variable helpers of the setup
//! dialog `TACalDlg` (ACalBox.cpp). The search, its memo of recent trials and
//! the score follow the original statement by statement, so the port tries
//! the same values in the same order and ends with the same antenna;
//! `tests/optimize.rs` checks this against the original C++
//! (`reference/oracle/optimize_main.cpp`).
//!
//! Like the element editor this is stateless: [`optimize`] takes the antenna
//! and the setup (MMANA's "最適化シート") and returns the optimised antenna and
//! the trials, [`apply_trial`] writes the values of one trial back, and
//! [`edit_setup`] replays edits of the variable table.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

#[cfg(feature = "openapi")]
use utoipa::ToSchema;

use crate::calc::{CalcOptions, Complex, PatternResolution};
use crate::elements::{ElementList, ElementRow, round_up};
use crate::engine::PAI;
use crate::error::CalcError;
use crate::model::{Antenna, Environment, GroundKind, LoadKind, Project, SPEED_OF_LIGHT_MMANA, Source, Wire};

/// `AVMAX`: the most variables a setup holds.
pub const MAX_VARIABLES: usize = 128;
/// `BMAX`: the most bands scored together.
pub const MAX_BANDS: usize = 10;
/// Sources a band sets (`BDEF::cdef`). Antennas with more sources are scored
/// on their own sources only.
pub const BAND_SOURCES: usize = 4;
/// `MVMAX`: trials remembered (and listed afterwards).
pub const TRIALS_KEPT: usize = 128;

// ---------------------------------------------------------------- setup ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum WirePart {
    Y1,
    Z1,
    Y2,
    Z2,
    Radius,
    /// Absolute X of the start (the element's position is added).
    X1,
    X2,
}

/// A wire in polar form (ワイヤ（極座標）): length, azimuth or zenith angle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum PolarQuantity {
    Length,
    /// 径度: angle from +X in the X-Y plane (degrees).
    Azimuth,
    /// 天頂角: angle from +Z (degrees).
    Zenith,
}

/// Which point of the wire stays put when its polar form changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum PolarAnchor {
    Start,
    End,
    Center,
}

/// What of an element varies; the `…All` parts scale every coordinate
/// instead of moving only the outermost ones (エレメント編集の「全ての座標を同一比率で変更」).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum ElementPart {
    Interval,
    Width,
    Height,
    /// Perimeter of a loop, or the extent along X of other forms.
    Perimeter,
    Radius,
    Position,
    WidthAll,
    HeightAll,
    PerimeterAll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum LoadPart {
    /// L (µH) of an LC load, R (Ω) of an R + jX load. Changing L or C of a
    /// trap (both set) keeps its resonant frequency.
    Primary,
    /// C (pF) of an LC load, jX (Ω) of an R + jX load.
    Secondary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum SourcePart {
    /// Degrees.
    Phase,
    /// Volts.
    Voltage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum StackPart {
    /// Both distances together (間隔).
    Both,
    Horizontal,
    Vertical,
}

/// What a variable changes. Indices are 0-based.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum VariableTarget {
    /// A wire coordinate or radius (metres). The other wires of its element
    /// that met it there follow.
    Wire {
        wire: usize,
        part: WirePart,
    },
    PolarWire {
        wire: usize,
        quantity: PolarQuantity,
        anchor: PolarAnchor,
    },
    Element {
        element: usize,
        part: ElementPart,
    },
    Load {
        load: usize,
        part: LoadPart,
    },
    /// Antenna height above ground (metres).
    Height,
    /// Calculation frequency (MHz).
    Frequency,
    Source {
        source: usize,
        part: SourcePart,
    },
    /// Stacking distance (metres).
    Stack {
        part: StackPart,
    },
}

impl VariableTarget {
    /// The original's `Type`, `Pos` and `SubPos`.
    pub fn code(&self) -> (u8, usize, u8) {
        use VariableTarget::*;
        match *self {
            Wire { wire, part } => (0, wire, part as u8),
            Element { element, part } => (1, element, part as u8),
            Load { load, part } => (2, load, part as u8),
            Height => (3, 0, 0),
            Frequency => (4, 0, 0),
            PolarWire { wire, quantity, anchor } => (5, wire, quantity as u8 * 3 + anchor as u8),
            Source { source, part } => (6, source, part as u8),
            Stack { part } => (7, 0, part as u8),
        }
    }

    /// The target of the original's `Type`, `Pos` and `SubPos`.
    pub fn from_code(kind: u8, pos: usize, sub: u8) -> Option<Self> {
        use VariableTarget::*;
        use {ElementPart as E, WirePart as W};
        let sub = sub as usize;
        Some(match kind {
            0 => Wire { wire: pos, part: *[W::Y1, W::Z1, W::Y2, W::Z2, W::Radius, W::X1, W::X2].get(sub)? },
            1 => {
                let parts = [
                    E::Interval,
                    E::Width,
                    E::Height,
                    E::Perimeter,
                    E::Radius,
                    E::Position,
                    E::WidthAll,
                    E::HeightAll,
                    E::PerimeterAll,
                ];
                Element { element: pos, part: *parts.get(sub)? }
            }
            2 => Load { load: pos, part: *[LoadPart::Primary, LoadPart::Secondary].get(sub)? },
            3 => Height,
            4 => Frequency,
            5 if sub < 9 => PolarWire {
                wire: pos,
                quantity: [PolarQuantity::Length, PolarQuantity::Azimuth, PolarQuantity::Zenith][sub / 3],
                anchor: [PolarAnchor::Start, PolarAnchor::End, PolarAnchor::Center][sub % 3],
            },
            6 => Source { source: pos, part: *[SourcePart::Phase, SourcePart::Voltage].get(sub)? },
            7 => Stack { part: *[StackPart::Both, StackPart::Horizontal, StackPart::Vertical].get(sub)? },
            _ => return None,
        })
    }
}

/// How a variable follows another one (連携). `variable` is the 0-based
/// index of the variable it follows; a chain of links ends at an independent
/// variable, and only the last link's arithmetic applies (as in MMANA).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Link {
    /// Varied by the search.
    #[default]
    Independent,
    /// `n`: the same value.
    Same { variable: usize },
    /// `-n`: the negated value.
    Negated { variable: usize },
    /// `n+c`
    Offset { variable: usize, offset: f64 },
    /// `-n+c`
    NegatedOffset { variable: usize, offset: f64 },
    /// `n*c`
    Scaled { variable: usize, factor: f64 },
    /// `n/c`
    Divided { variable: usize, divisor: f64 },
}

impl Link {
    pub fn followed(&self) -> Option<usize> {
        match *self {
            Link::Independent => None,
            Link::Same { variable }
            | Link::Negated { variable }
            | Link::Offset { variable, .. }
            | Link::NegatedOffset { variable, .. }
            | Link::Scaled { variable, .. }
            | Link::Divided { variable, .. } => Some(variable),
        }
    }

    fn with_variable(self, n: usize) -> Link {
        match self {
            Link::Independent => Link::Independent,
            Link::Same { .. } => Link::Same { variable: n },
            Link::Negated { .. } => Link::Negated { variable: n },
            Link::Offset { offset, .. } => Link::Offset { variable: n, offset },
            Link::NegatedOffset { offset, .. } => Link::NegatedOffset { variable: n, offset },
            Link::Scaled { factor, .. } => Link::Scaled { variable: n, factor },
            Link::Divided { divisor, .. } => Link::Divided { variable: n, divisor },
        }
    }

    /// The value for a followed value `v` (`GetNowValue(n, sc)` / `SetPara(n)`).
    fn apply(&self, v: f64) -> f64 {
        match *self {
            Link::Independent | Link::Same { .. } => v,
            Link::Negated { .. } => -v,
            Link::Offset { offset, .. } => v + offset,
            Link::NegatedOffset { offset, .. } => -v + offset,
            Link::Scaled { factor, .. } => v * factor,
            Link::Divided { divisor, .. } => v / divisor,
        }
    }
}

/// One row of the variable table (`AVAL`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Variable {
    pub target: VariableTarget,
    #[serde(default)]
    pub link: Link,
    /// Smallest step tried (ピッチ): in the variable's unit, or in percent of
    /// its starting value when the setup's `absolutePitch` is off.
    pub pitch: f64,
    pub min: f64,
    pub max: f64,
    /// Use the current value of this variable as the minimum instead (`#n`).
    #[serde(default)]
    pub min_from: Option<usize>,
    #[serde(default)]
    pub max_from: Option<usize>,
}

impl Variable {
    fn new(target: VariableTarget) -> Self {
        Self { target, link: Link::Independent, pitch: 0.0, min: 0.0, max: 0.0, min_from: None, max_from: None }
    }
}

/// How much each goal counts (評価の割合, 0-100 each). All zero counts every
/// goal equally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase", default)]
pub struct Weights {
    pub gain: u32,
    pub fb: u32,
    pub jx: u32,
    pub swr: u32,
    pub elevation: u32,
    pub matching: u32,
    pub current: u32,
}

impl Default for Weights {
    fn default() -> Self {
        Self { gain: 50, fb: 50, jx: 50, swr: 0, elevation: 0, matching: 0, current: 0 }
    }
}

impl Weights {
    /// `SetTradeOff`: `[gain, fb, jx, swr, elevation, matching, current]`.
    fn trade_off(&self) -> [f64; 7] {
        let w = [self.gain, self.fb, self.jx, self.swr, self.elevation, self.matching, self.current];
        if w.iter().all(|&x| x == 0) { [1.0; 7] } else { w.map(f64::from) }
    }
}

/// The matching network scored by the `matching` goal (マッチング回路).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Matching {
    /// キャパシタンスマッチ: wants an inductive jX.
    #[default]
    Capacitance,
    /// ヘアピンマッチ: wants a capacitive jX.
    Hairpin,
    /// 任意のZ: wants R + jX; `null` leaves that part unscored (`*`).
    Impedance { r: Option<f64>, x: Option<f64> },
}

/// Limits of the goals (目標詳細設定).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase", default)]
pub struct Goals {
    /// Gain above this counts little more (許容最大目標).
    pub max_gain_dbi: f64,
    pub max_fb_db: f64,
    /// |jX| below this counts little less.
    pub min_jx: f64,
    pub min_swr: f64,
    pub matching: Matching,
    /// Pulse of the current goal (`w1c`, …).
    pub current_pulse: String,
    pub minimize_current: bool,
}

impl Default for Goals {
    fn default() -> Self {
        Self {
            max_gain_dbi: 100.0,
            max_fb_db: 25.0,
            min_jx: 0.0,
            min_swr: 1.0,
            matching: Matching::Capacitance,
            current_pulse: "w1c".into(),
            minimize_current: false,
        }
    }
}

/// A band scored with the others (試行バンド): frequency and sources.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Band {
    pub frequency_mhz: f64,
    pub sources: Vec<Source>,
}

/// MMANA's optimisation sheet (最適化シート) without its trial list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase", default)]
pub struct OptimizerSetup {
    pub variables: Vec<Variable>,
    pub weights: Weights,
    /// No goals (目標なし（単純スイープ）): step each variable from its value
    /// up to its maximum and back down to its minimum, recording every trial.
    pub sweep_only: bool,
    /// Pitches are in the variables' units (可変ﾋﾟｯﾁは絶対値), not percent.
    pub absolute_pitch: bool,
    /// 2° patterns (分解能２°), about twice as fast.
    pub coarse_pattern: bool,
    pub goals: Goals,
    /// Bands scored together. With fewer than two, the antenna is scored at
    /// its calculation frequency with its own sources.
    pub bands: Vec<Band>,
    /// Band 1 counts fully and the others 1 / band count each (１番を重視);
    /// otherwise all bands add up.
    pub emphasize_first_band: bool,
}

impl Default for OptimizerSetup {
    fn default() -> Self {
        Self {
            variables: Vec::new(),
            weights: Weights::default(),
            sweep_only: false,
            absolute_pitch: true,
            coarse_pattern: true,
            goals: Goals::default(),
            bands: Vec::new(),
            emphasize_first_band: true,
        }
    }
}

// ------------------------------------------------------------- results ---

/// One calculation of the search (a line of MMANA's optimisation log).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    /// 1-based count of calculations so far.
    pub calculation: usize,
    /// The variable being varied (0-based).
    pub variable: usize,
    pub band: usize,
    pub frequency_mhz: f64,
    pub impedance: Complex,
    pub swr: f32,
    pub gain_dbi: f32,
    pub fb_db: f32,
    pub elevation_deg: f32,
    /// |I| (A) at the current goal's pulse, when that goal is weighted.
    pub current: Option<f32>,
    /// The trial's score, after its last band.
    pub score: Option<f64>,
    /// The trial scored better than any before (after its last band).
    pub improved: bool,
    /// Current value of every variable.
    pub values: Vec<f64>,
}

/// A remembered trial (`MVAL`). Figures are of band 1.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Trial {
    /// Value of every variable.
    pub values: Vec<f64>,
    pub score: f64,
    pub impedance: Complex,
    pub swr: f32,
    pub gain_dbi: f32,
    pub fb_db: f32,
    pub elevation_deg: f32,
    /// Total gain (dBi) every 2° of azimuth, from 0°.
    pub horizontal_pattern: Vec<f32>,
    /// Total gain (dBi) every 2° of the vertical cut (as `Patterns::vertical`).
    pub vertical_pattern: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum StopReason {
    /// A whole pass improved nothing (or the sweep finished).
    Converged,
    Cancelled,
    /// The calculation limit of the request was reached.
    CalculationLimit,
    /// A calculation failed; the best antenna so far is kept.
    Failed {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct OptimizeOutcome {
    /// The antenna with the best values (unchanged when no trial finished).
    pub project: Project,
    /// Calculation frequency afterwards (changed by a frequency variable).
    pub frequency_mhz: f64,
    pub stop: StopReason,
    pub calculations: usize,
    pub best: Option<Trial>,
    /// The last trials, newest first.
    pub trials: Vec<Trial>,
    pub warnings: Vec<SetupIssue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum IssueCode {
    NoVariables,
    TooManyVariables,
    /// The wire, element, load or source does not exist.
    MissingTarget,
    /// Starts at 0, which a percentage pitch or the element cannot vary from.
    ZeroValue,
    /// S-domain loads cannot vary.
    SDomainLoad,
    /// The height means nothing in free space.
    HeightInFreeSpace,
    /// Varies the same thing as variable `other`.
    Duplicate,
    /// The link (or a `#` bound) refers to a variable that does not exist.
    MissingVariable,
    /// The links go round without reaching an independent variable.
    LinkLoop,
    TooManyBands,
    /// Band `other` has no source.
    BandWithoutSources,
    /// The source the variable varies does not exist in band `other`.
    BandMissingSource,
    /// Warning: the value differs from what its link gives (the link wins).
    LinkValueDiffers,
}

/// Something in the setup that stops a run, or a warning.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SetupIssue {
    pub code: IssueCode,
    pub variable: Option<usize>,
    pub other: Option<usize>,
    pub message: String,
}

impl SetupIssue {
    fn new(code: IssueCode, variable: Option<usize>, other: Option<usize>) -> Self {
        let v = |n: Option<usize>| n.map_or(0, |n| n + 1);
        let message = match code {
            IssueCode::NoVariables => "no variables".to_owned(),
            IssueCode::TooManyVariables => format!("at most {MAX_VARIABLES} variables"),
            IssueCode::MissingTarget => format!("variable {}: what it varies does not exist", v(variable)),
            IssueCode::ZeroValue => format!("variable {}: cannot vary from 0", v(variable)),
            IssueCode::SDomainLoad => format!("variable {}: S-domain loads cannot vary", v(variable)),
            IssueCode::HeightInFreeSpace => format!("variable {}: the height means nothing in free space", v(variable)),
            IssueCode::Duplicate => format!("variable {}: same as variable {}", v(variable), v(other)),
            IssueCode::MissingVariable => format!("variable {}: variable {} does not exist", v(variable), v(other)),
            IssueCode::LinkLoop => format!("variable {}: its links never reach an independent variable", v(variable)),
            IssueCode::TooManyBands => format!("at most {MAX_BANDS} bands"),
            IssueCode::BandWithoutSources => format!("band {} has no source", v(other)),
            IssueCode::BandMissingSource => {
                format!("variable {}: band {} has no such source", v(variable), v(other))
            }
            IssueCode::LinkValueDiffers => {
                format!("variable {}: its value differs from variable {}'s; the link sets it", v(variable), v(other))
            }
        };
        Self { code, variable, other, message }
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum OptimizeError {
    #[error("{}", .0.iter().map(|i| i.message.as_str()).collect::<Vec<_>>().join("; "))]
    Setup(Vec<SetupIssue>),
    #[error("expected {expected} values, got {got}")]
    ValueCount { expected: usize, got: usize },
}

// ---------------------------------------------------------- arithmetic ---

/// `RoundUpStr`: through `"%lf"` (6 decimals) and back.
fn round_up_str(d: f64) -> f64 {
    format!("{d:.6}").parse().unwrap_or(d)
}

/// `GetNormal`: the leading digit of `d`, 6 to 9 rounded up to 10.
fn get_normal(mut d: f64) -> f64 {
    if d == 0.0 {
        return d;
    }
    let mut n = 0;
    while d.abs() < 1.0 {
        d *= 10.0;
        n += 1;
    }
    d = d.trunc();
    if d > 5.0 {
        d = 10.0;
    }
    for _ in 0..n {
        d /= 10.0;
    }
    d
}

/// `GetDeg`
fn get_deg(x: f64, y: f64) -> f64 {
    let mut deg = if x > 0.0 {
        (y / x).atan() * 180.0 / PAI
    } else if x < 0.0 {
        180.0 - (y / -x).atan() * 180.0 / PAI
    } else if y > 0.0 {
        90.0
    } else if y < 0.0 {
        -90.0
    } else {
        0.0
    };
    if deg > 180.0 {
        deg -= 360.0;
    }
    if deg < -180.0 {
        deg += 360.0;
    }
    deg
}

/// `GetWirePoll`: length, azimuth and zenith angle.
fn polar(w: &Wire) -> (f64, f64, f64) {
    let xx = w.end.x - w.start.x;
    let yy = w.end.y - w.start.y;
    let zz = w.end.z - w.start.z;
    let wl = (xx * xx + yy * yy + zz * zz).sqrt();
    let mut xyd = get_deg(xx, yy);
    let mut yzd = get_deg(zz, (xx * xx + yy * yy).sqrt());
    while xyd > 180.0 {
        xyd -= 360.0;
    }
    while xyd < -180.0 {
        xyd += 360.0;
    }
    while yzd > 180.0 {
        yzd -= 360.0;
    }
    while yzd < -180.0 {
        yzd += 360.0;
    }
    (wl, xyd, yzd)
}

/// `A[0]` and `B[0]` of a load: L and C, R and jX, or the s⁰ coefficients.
fn load_values(kind: &LoadKind) -> (f64, f64) {
    match kind {
        LoadKind::Lc { inductance_uh, capacitance_pf, .. } => (*inductance_uh, *capacitance_pf),
        LoadKind::Impedance { r, x } => (*r, *x),
        LoadKind::SDomain { coefficients } => coefficients.first().map_or((0.0, 0.0), |c| (c[0], c[1])),
    }
}

// ---------------------------------------------------------------- model ---

/// The antenna as the optimiser changes it: wires through the element table
/// (`CACal::WList`), the rest in place (`AP`, `EP`).
#[derive(Clone)]
struct Model {
    list: ElementList,
    antenna: Antenna,
    environment: Environment,
    /// `AP->cfq`
    frequency: f64,
    /// `atrg.bdef`
    bands: Vec<Band>,
}

impl Model {
    /// `CACal::Init` (`WList.SetWire`).
    fn new(project: &Project, options: &CalcOptions) -> Self {
        let ant = &project.antenna;
        let f = ant.frequency_mhz;
        let wavelength = if f != 0.0 { SPEED_OF_LIGHT_MMANA / f } else { 0.0 };
        let frequency = match options.frequency_mhz {
            Some(f) if f != 0.0 => f,
            _ => f,
        };
        Self {
            list: ElementList::new(&ant.wires, wavelength),
            antenna: ant.clone(),
            environment: project.environment.clone(),
            frequency,
            bands: Vec::new(),
        }
    }

    fn exists(&self, t: &VariableTarget) -> bool {
        use VariableTarget::*;
        match *t {
            Wire { wire, .. } | PolarWire { wire, .. } => wire < self.list.wire_count(),
            Element { element, .. } => element < self.list.len(),
            Load { load, .. } => load < self.antenna.loads.len(),
            Source { source, .. } => source < self.antenna.sources.len(),
            Height | Frequency | Stack { .. } => true,
        }
    }

    /// How many things of the target's kind exist (`AdjSetup`'s limit).
    fn count(&self, t: &VariableTarget) -> usize {
        use VariableTarget::*;
        match t {
            Wire { .. } | PolarWire { .. } => self.list.wire_count(),
            Element { .. } => self.list.len(),
            Load { .. } => self.antenna.loads.len(),
            Source { .. } => self.antenna.sources.len(),
            Height | Frequency | Stack { .. } => 1,
        }
    }

    /// `GetNowValue(n)`
    fn value(&self, t: &VariableTarget) -> f64 {
        use VariableTarget as T;
        match *t {
            T::Wire { wire, part } => {
                let w = self.list.wire(wire);
                match part {
                    WirePart::Y1 => w.start.y,
                    WirePart::Z1 => w.start.z,
                    WirePart::Y2 => w.end.y,
                    WirePart::Z2 => w.end.z,
                    WirePart::Radius => w.radius,
                    WirePart::X1 => w.start.x + self.list.x_abs(wire),
                    WirePart::X2 => w.end.x + self.list.x_abs(wire),
                }
            }
            T::PolarWire { wire, quantity, .. } => {
                let (wl, xyd, yzd) = polar(self.list.wire(wire));
                match quantity {
                    PolarQuantity::Length => wl,
                    PolarQuantity::Azimuth => xyd,
                    PolarQuantity::Zenith => yzd,
                }
            }
            T::Element { element: e, part } => {
                let l = &self.list;
                match part {
                    ElementPart::Interval => l.element_interval(e),
                    ElementPart::Width | ElementPart::WidthAll => l.element_width(e),
                    ElementPart::Height | ElementPart::HeightAll => l.element_height(e),
                    ElementPart::Perimeter | ElementPart::PerimeterAll => l.element_perimeter(e),
                    ElementPart::Radius => l.element_radius(e),
                    ElementPart::Position => l.element_position(e),
                }
            }
            T::Load { load, part } => {
                let (a, b) = load_values(&self.antenna.loads[load].kind);
                if part == LoadPart::Secondary { b } else { a }
            }
            T::Height => self.environment.height,
            T::Frequency => self.frequency,
            // (a band may have fewer sources; `Check` refuses variables on those)
            T::Source { source, part } => self
                .antenna
                .sources
                .get(source)
                .map_or(0.0, |s| if part == SourcePart::Voltage { s.voltage } else { s.phase_deg }),
            T::Stack { part } => {
                let s = &self.antenna.stack;
                if part == StackPart::Vertical { s.vertical_spacing } else { s.horizontal_spacing }
            }
        }
    }

    /// `SetPara(n, d)` without the bookkeeping.
    fn set(&mut self, t: &VariableTarget, d: f64) {
        use VariableTarget as T;
        match *t {
            T::Wire { wire, part } => {
                // `SetParaWire`
                let old = self.list.wire(wire).clone();
                let x_abs = if matches!(part, WirePart::X1 | WirePart::X2) { self.list.x_abs(wire) } else { 0.0 };
                let w = self.list.wire_mut(wire);
                match part {
                    WirePart::Y1 => w.start.y = d,
                    WirePart::Z1 => w.start.z = d,
                    WirePart::Y2 => w.end.y = d,
                    WirePart::Z2 => w.end.z = d,
                    WirePart::Radius => w.radius = d,
                    WirePart::X1 => w.start.x = d - x_abs,
                    WirePart::X2 => w.end.x = d - x_abs,
                }
                self.list.follow_wire(wire, &old);
            }
            T::PolarWire { wire, quantity, anchor } => {
                // `SetParaWireP`
                let old = self.list.wire(wire).clone();
                let (mut wl, mut xyd, mut yzd) = polar(&old);
                match quantity {
                    PolarQuantity::Length => wl = d,
                    PolarQuantity::Azimuth => xyd = d,
                    PolarQuantity::Zenith => yzd = d,
                }
                let mut xx = wl * (yzd * PAI / 180.0).sin() * (xyd * PAI / 180.0).cos();
                let mut yy = wl * (yzd * PAI / 180.0).sin() * (xyd * PAI / 180.0).sin();
                let mut zz = wl * (yzd * PAI / 180.0).cos();
                let xc = (old.start.x + old.end.x) / 2.0;
                let yc = (old.start.y + old.end.y) / 2.0;
                let zc = (old.start.z + old.end.z) / 2.0;
                let w = self.list.wire_mut(wire);
                match anchor {
                    PolarAnchor::Start => {
                        w.end.x = w.start.x + xx;
                        w.end.y = w.start.y + yy;
                        w.end.z = w.start.z + zz;
                    }
                    PolarAnchor::End => {
                        w.start.x = w.end.x - xx;
                        w.start.y = w.end.y - yy;
                        w.start.z = w.end.z - zz;
                    }
                    PolarAnchor::Center => {
                        xx /= 2.0;
                        yy /= 2.0;
                        zz /= 2.0;
                        w.start.x = xc - xx;
                        w.start.y = yc - yy;
                        w.start.z = zc - zz;
                        w.end.x = xc + xx;
                        w.end.y = yc + yy;
                        w.end.z = zc + zz;
                    }
                }
                self.list.follow_wire(wire, &old);
            }
            T::Element { element: e, part } => {
                // `SetParaEle`
                let l = &mut self.list;
                match part {
                    ElementPart::Interval => l.set_element_interval(e, d),
                    ElementPart::Width => l.set_element_width(e, false, d),
                    ElementPart::Height => l.set_element_height(e, false, d),
                    ElementPart::Perimeter => l.set_element_perimeter(e, false, d),
                    ElementPart::Radius => l.set_element_radius(e, d),
                    ElementPart::Position => l.set_element_position(e, d),
                    ElementPart::WidthAll => l.set_element_width(e, true, d),
                    ElementPart::HeightAll => l.set_element_height(e, true, d),
                    ElementPart::PerimeterAll => l.set_element_perimeter(e, true, d),
                }
            }
            T::Load { load, part } => {
                // `SetParaLoad`: changing L or C of a trap keeps its resonance
                let second = part == LoadPart::Secondary;
                let set = |a: &mut f64, b: &mut f64, trap: bool| {
                    if trap && *a != 0.0 && *b != 0.0 {
                        let w2 = 1.0 / (*a * *b);
                        if second {
                            *b = d;
                            *a = 1.0 / (w2 * d);
                        } else {
                            *a = d;
                            *b = 1.0 / (w2 * d);
                        }
                    } else if second {
                        *b = d;
                    } else {
                        *a = d;
                    }
                };
                match &mut self.antenna.loads[load].kind {
                    LoadKind::Lc { inductance_uh, capacitance_pf, .. } => set(inductance_uh, capacitance_pf, true),
                    LoadKind::Impedance { r, x } => set(r, x, false),
                    LoadKind::SDomain { coefficients } => {
                        if let Some([a, b]) = coefficients.first_mut() {
                            set(a, b, false);
                        }
                    }
                }
            }
            T::Height => self.environment.height = d,
            T::Frequency => {
                self.frequency = d;
                if let Some(b) = self.bands.first_mut() {
                    b.frequency_mhz = d;
                }
            }
            T::Source { source, part } => {
                if let Some(s) = self.antenna.sources.get_mut(source) {
                    if part == SourcePart::Voltage {
                        s.voltage = d;
                    } else {
                        s.phase_deg = d;
                    }
                }
            }
            T::Stack { part } => {
                let s = &mut self.antenna.stack;
                match part {
                    StackPart::Both => {
                        s.horizontal_spacing = d;
                        s.vertical_spacing = d;
                    }
                    StackPart::Horizontal => s.horizontal_spacing = d,
                    StackPart::Vertical => s.vertical_spacing = d,
                }
            }
        }
    }

    /// `SetBand`: frequency and sources of band `n`.
    fn set_band(&mut self, n: usize) {
        if self.antenna.sources.len() > BAND_SOURCES {
            return;
        }
        let b = &self.bands[n];
        self.frequency = b.frequency_mhz;
        self.antenna.sources = band_sources(b).cloned().collect();
    }

    /// `WList.Update`: writes the element table's wires back.
    fn update_wires(&mut self) {
        self.antenna.wires = self.list.wires().wires;
    }

    fn project(&self) -> Project {
        Project { antenna: self.antenna.clone(), environment: self.environment.clone() }
    }
}

// ---------------------------------------------------------------- check ---

/// The search's per-variable state (`AVAL::DI`, `DIO`, `DC`).
#[derive(Debug, Clone, Copy, Default)]
struct VarState {
    /// Step now (`DI`).
    di: f64,
    /// Starting step (`DIO`).
    dio: f64,
    /// Value now (`DC`).
    dc: f64,
}

/// `SetMinMax`: pulls `d` back inside the bounds of variable `n`; true when
/// it was outside.
fn clamp(model: &Model, vars: &[Variable], state: &[VarState], n: usize, d: &mut f64) -> bool {
    let v = &vars[n];
    if v.link != Link::Independent {
        return false;
    }
    if let Some(k) = v.max_from {
        let dd = model.value(&vars[k].target);
        if *d > dd {
            *d = dd;
            return true;
        }
    } else if *d >= v.max {
        *d = v.max - state[n].di;
        return true;
    }
    if let Some(k) = v.min_from {
        let dd = model.value(&vars[k].target);
        if *d < dd {
            *d = dd;
            return true;
        }
    } else if *d <= v.min {
        *d = v.min + state[n].di;
        return true;
    }
    false
}

/// `GetLastChen`: the independent variable at the end of `n`'s links.
fn last_in_chain(vars: &[Variable], n: usize) -> Result<usize, (IssueCode, Option<usize>)> {
    let mut k = vars[n].link.followed().expect("a dependent variable");
    for _ in 0..=vars.len() {
        let Some(v) = vars.get(k) else { return Err((IssueCode::MissingVariable, Some(k))) };
        match v.link.followed() {
            None => return Ok(k),
            Some(next) => k = next,
        }
    }
    Err((IssueCode::LinkLoop, None))
}

/// The sources `SetBand` gives the antenna: up to the first blank pulse, at most four.
fn band_sources(b: &Band) -> impl Iterator<Item = &Source> {
    b.sources.iter().take_while(|s| !s.pulse.trim().is_empty()).take(BAND_SOURCES)
}

/// Whether the bands of the setup are scored (`bmax` > 1).
fn uses_bands(antenna: &Antenna, setup: &OptimizerSetup) -> bool {
    !setup.sweep_only && antenna.sources.len() <= BAND_SOURCES && setup.bands.len() >= 2
}

/// `Check`: what cannot be varied, and the independent variables clamped
/// into their bounds (this changes `model`). It runs with no step yet, as on
/// the original's first run, so a value at or beyond a bound lands on it.
fn check(model: &mut Model, setup: &OptimizerSetup) -> (Vec<SetupIssue>, Vec<SetupIssue>) {
    let vars = &setup.variables;
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let issue = |code, n: usize, other: Option<usize>| SetupIssue::new(code, Some(n), other);
    if vars.is_empty() {
        errors.push(SetupIssue::new(IssueCode::NoVariables, None, None));
    }
    if vars.len() > MAX_VARIABLES {
        errors.push(SetupIssue::new(IssueCode::TooManyVariables, None, None));
    }
    if uses_bands(&model.antenna, setup) {
        if setup.bands.len() > MAX_BANDS {
            errors.push(SetupIssue::new(IssueCode::TooManyBands, None, None));
        }
        for (b, band) in setup.bands.iter().enumerate() {
            let count = band_sources(band).count();
            if count == 0 {
                errors.push(SetupIssue::new(IssueCode::BandWithoutSources, None, Some(b)));
            }
            // each band sets the antenna's sources, so a source variable must
            // exist in every band (the original writes past the last one)
            for (i, v) in vars.iter().enumerate() {
                if let VariableTarget::Source { source, .. } = v.target
                    && source >= count
                {
                    errors.push(issue(IssueCode::BandMissingSource, i, Some(b)));
                }
            }
        }
    }
    let state = vec![VarState::default(); vars.len()];
    let usable = |model: &Model, k: usize| vars.get(k).is_some_and(|w| model.exists(&w.target));
    for (i, v) in vars.iter().enumerate() {
        if !model.exists(&v.target) {
            errors.push(issue(IssueCode::MissingTarget, i, None));
            continue;
        }
        let mut d = model.value(&v.target);
        match v.target {
            VariableTarget::Element { part, .. } if d == 0.0 && part != ElementPart::Position => {
                errors.push(issue(IssueCode::ZeroValue, i, None));
            }
            VariableTarget::Load { load, .. } if matches!(model.antenna.loads[load].kind, LoadKind::SDomain { .. }) => {
                errors.push(issue(IssueCode::SDomainLoad, i, None));
            }
            VariableTarget::Height if model.environment.ground == GroundKind::FreeSpace => {
                errors.push(issue(IssueCode::HeightInFreeSpace, i, None));
            }
            _ => {}
        }
        let code = v.target.code();
        if let Some(j) = vars.iter().enumerate().position(|(j, w)| j != i && w.target.code() == code) {
            errors.push(issue(IssueCode::Duplicate, i, Some(j)));
        }
        if let Some(k) = v.link.followed() {
            if usable(model, k) {
                if round_up_str(d) != round_up_str(v.link.apply(model.value(&vars[k].target))) {
                    warnings.push(issue(IssueCode::LinkValueDiffers, i, Some(k)));
                }
            } else if vars.get(k).is_none() {
                errors.push(issue(IssueCode::MissingVariable, i, Some(k)));
            }
        } else {
            let bounds = [v.min_from, v.max_from];
            let missing: Vec<usize> = bounds.into_iter().flatten().filter(|&k| !usable(model, k)).collect();
            if missing.is_empty() {
                if clamp(model, vars, &state, i, &mut d) {
                    model.set(&v.target, d);
                }
            } else {
                errors.extend(missing.into_iter().map(|k| issue(IssueCode::MissingVariable, i, Some(k))));
            }
        }
    }
    (errors, warnings)
}

/// The variables ready for the search.
struct Prepared {
    /// Links resolved to independent variables.
    vars: Vec<Variable>,
    state: Vec<VarState>,
    /// `AD`: the best values, set apart from the starting ones.
    ad: Vec<f64>,
}

/// `InitPara`: resolves the links to independent variables and sets the
/// starting values and steps.
fn init_para(model: &Model, setup: &OptimizerSetup) -> Result<Prepared, Vec<SetupIssue>> {
    let mut vars = setup.variables.clone();
    let mut state = vec![VarState::default(); vars.len()];
    let mut ad = vec![0.0; vars.len()];
    let mut errors = Vec::new();
    for i in 0..vars.len() {
        if vars[i].link != Link::Independent {
            match last_in_chain(&setup.variables, i) {
                Ok(k) => vars[i].link = vars[i].link.with_variable(k),
                Err((code, other)) => errors.push(SetupIssue::new(code, Some(i), other)),
            }
            continue;
        }
        let dc = model.value(&vars[i].target);
        let fixed = match vars[i].target {
            VariableTarget::Element { part, .. } => (part as u8) < ElementPart::Position as u8,
            VariableTarget::Load { .. } => true,
            _ => false,
        };
        if dc == 0.0 && fixed {
            errors.push(SetupIssue::new(IssueCode::ZeroValue, Some(i), None));
            continue;
        }
        ad[i] = -dc;
        let di = if setup.absolute_pitch { vars[i].pitch } else { (dc * vars[i].pitch) / 100.0 };
        let di = if di < 0.0 { -di } else { di };
        state[i] = VarState { di, dio: di, dc };
    }
    if errors.is_empty() { Ok(Prepared { vars, state, ad }) } else { Err(errors) }
}

/// Everything that would stop a run, and the warnings.
fn issues(model: &Model, setup: &OptimizerSetup) -> (Vec<SetupIssue>, Vec<SetupIssue>) {
    let mut model = model.clone();
    let (mut errors, warnings) = check(&mut model, setup);
    if errors.is_empty()
        && let Err(e) = init_para(&model, setup)
    {
        errors = e;
    }
    (errors, warnings)
}

// --------------------------------------------------------------- search ---

/// The figures of one calculation (`RESL`, which holds them as `float`).
#[derive(Debug, Clone)]
struct Reading {
    r: f32,
    jx: f32,
    swr: f32,
    gain: f32,
    fb: f32,
    elv: f32,
    /// `ABS(Cur[n-1])` at the current goal's pulse.
    current: Option<f32>,
    horizontal: Vec<f32>,
    vertical: Vec<f32>,
}

/// `ABS()` of the original, which keeps the sign of -0.
fn c_abs(c: f64) -> f64 {
    if c < 0.0 { -c } else { c }
}

fn c_abs32(c: f32) -> f32 {
    if c < 0.0 { -c } else { c }
}

enum Stop {
    Cancelled,
    Failed(CalcError),
}

/// `CACal` with its goal state (`ATRG`).
struct Search<'a, F> {
    model: Model,
    vars: Vec<Variable>,
    state: Vec<VarState>,
    setup: &'a OptimizerSetup,
    options: CalcOptions,
    /// `TG`, `TFB`, `TJX`, `TSWR`, `TELV`, `TM`, `TI`
    trade: [f64; 7],
    /// `bmax`
    bands: usize,
    /// `V`: score of the trial now.
    v: f64,
    /// `AM`: best score while varying the variable now.
    am: f64,
    /// `VM`: best score.
    vm: f64,
    /// `AD`: values at the best score.
    ad: Vec<f64>,
    /// `mval`, newest first.
    trials: VecDeque<Trial>,
    best: Option<Trial>,
    calculations: usize,
    limit: Option<usize>,
    /// `exeenv.CalcAbort`
    abort: bool,
    limited: bool,
    progress: F,
}

impl<F: FnMut(&Progress) -> bool> Search<'_, F> {
    fn independent(&self, n: usize) -> bool {
        self.vars[n].link == Link::Independent
    }

    fn values(&self) -> Vec<f64> {
        self.state.iter().map(|s| s.dc).collect()
    }

    /// `SetPara(n, d)`
    fn set_para(&mut self, n: usize, d: f64) {
        self.state[n].dc = d;
        self.model.set(&self.vars[n].target, d);
    }

    /// `SetPara(n)`: an independent variable and the variables linked to it.
    fn set_para_linked(&mut self, n: usize) {
        if !self.independent(n) {
            return;
        }
        let v = self.state[n].dc;
        self.set_para(n, v);
        for i in 0..self.vars.len() {
            let link = self.vars[i].link;
            if link.followed() == Some(n) {
                self.set_para(i, link.apply(v));
            }
        }
    }

    /// `SetMinMax` on the value of variable `n`.
    fn clamp(&self, n: usize, d: &mut f64) -> bool {
        clamp(&self.model, &self.vars, &self.state, n, d)
    }

    /// `IsSameMax`: the values are those of the best trial.
    fn is_same_max(&self) -> bool {
        self.state.iter().zip(&self.ad).all(|(s, &d)| s.dc == d)
    }

    /// `IsMV`: the score of a remembered trial with the values now.
    fn remembered(&self) -> Option<f64> {
        self.trials.iter().find(|t| t.values.iter().zip(&self.state).all(|(&d, s)| d == s.dc)).map(|t| t.score)
    }

    /// `ACalExecute`: calculates the antenna as it is now.
    fn calculate(&self) -> Result<Reading, CalcError> {
        let project = self.model.project();
        let opts = CalcOptions { frequency_mhz: Some(self.model.frequency), ..self.options.clone() };
        let solved = crate::calc::solve(&project, &opts)?;
        let r = solved.result(&project, &opts);
        let goal = &self.setup.goals.current_pulse;
        let current = if self.trade[6] != 0.0 && !goal.is_empty() {
            let n = solved.pulse_no(goal);
            (n >= 1).then(|| r.current_display.get(n as usize - 1).map(|&c| c_abs32(c))).flatten()
        } else {
            None
        };
        // the 2° patterns `WriteMV` keeps
        let every = |cut: &[f32]| {
            let n = (2.0 / r.patterns.horizontal.step_deg) as usize;
            cut.iter().step_by(n.max(1)).copied().collect::<Vec<f32>>()
        };
        Ok(Reading {
            r: r.impedance.re as f32,
            jx: r.impedance.im as f32,
            swr: r.swr as f32,
            gain: r.gain_dbi,
            fb: r.fb_db,
            elv: r.elevation_deg,
            current,
            horizontal: every(&r.patterns.horizontal.total),
            vertical: every(&r.patterns.vertical.total),
        })
    }

    /// `CalcV`: the score of one band.
    fn score(&self, rd: &Reading) -> f64 {
        if self.setup.sweep_only {
            return self.v - 0.1;
        }
        let g = &self.setup.goals;
        let [tg, tfb, tjx, tswr, telv, tm, ti] = self.trade;
        let mut fb = rd.fb as f64;
        if fb > g.max_fb_db {
            fb = g.max_fb_db + (fb / g.max_fb_db).ln();
        }
        fb *= tfb;
        let mut gain = rd.gain as f64;
        if gain > g.max_gain_dbi {
            gain = g.max_gain_dbi + (gain / g.max_gain_dbi).ln();
        }
        gain *= tg;
        let mut jx = c_abs32(rd.jx) as f64;
        if jx < g.min_jx {
            jx = (g.min_jx - ((g.min_jx - jx) / 20.0)) * tjx;
        } else {
            jx *= tjx;
        }
        jx = -jx;
        let mut swr = rd.swr as f64;
        if swr < g.min_swr {
            swr = (g.min_swr - ((g.min_swr - swr) / 20.0)) * tswr;
        } else {
            swr = (swr - 1.0) * tswr;
        }
        swr = -swr;
        let elv = -(rd.elv as f64 * telv);
        let mut mch = 0.0;
        if tm != 0.0 {
            let ro = self.model.environment.swr_reference.r;
            let (r, x) = (rd.r, rd.jx);
            let (r64, x64) = (r as f64, x as f64);
            match g.matching {
                Matching::Impedance { r: tr, x: tx } => {
                    let s = tr.map_or(1.0, |t| r64 - t);
                    mch = tx.map_or(1.0, |t| x64 - t);
                    mch = -(4.0 * s * s + mch * mch).sqrt();
                }
                _ if r64 <= ro => {
                    let s = ((ro / r64) - 1.0).sqrt();
                    mch = r64 * s;
                    if g.matching == Matching::Hairpin {
                        mch += x64;
                    } else {
                        mch -= x64;
                    }
                    mch = -c_abs(mch);
                }
                _ => {
                    // `R*R + JX*JX` of the `float`s is `float` arithmetic
                    let s = 4.0 * ro * ro * x64 * x64 - 4.0 * (ro - r64) * ro * (r * r + x * x) as f64;
                    if s >= 0.0 {
                        let s = s.sqrt();
                        let b = -2.0 * ro * x64;
                        let v = 2.0 * (ro - r64);
                        let mx1 = (b + s) / v;
                        let mx2 =
                            -(mx1 * r64 * r64 + (x64 + mx1) * x64 * mx1) / ((r * r) as f64 + (x64 + mx1) * (x64 + mx1));
                        mch = -c_abs(mx2);
                    } else {
                        mch = -r64;
                    }
                    mch -= 100.0;
                }
            }
            mch *= tm;
        }
        let mut cur = 0.0;
        if ti != 0.0
            && !g.current_pulse.is_empty()
            && let Some(c) = rd.current
        {
            cur = if g.minimize_current { -(c as f64) } else { c as f64 };
            cur *= ti * 100.0;
        }
        fb /= 10.0;
        jx /= 50.0;
        mch /= 50.0;
        gain + fb + jx + swr + elv + mch + cur
    }

    /// One trial over every band; true when it is the best so far.
    fn trial(&mut self, n: usize) -> Result<bool, Stop> {
        self.model.update_wires();
        self.v = 0.0;
        let mut bv = vec![0.0; self.bands];
        let mut first = None;
        for b in 0..self.bands {
            if self.bands != 1 {
                self.model.set_band(b);
            }
            let rd = self.calculate().map_err(Stop::Failed)?;
            bv[b] = self.score(&rd);
            let last = b == self.bands - 1;
            if last {
                // (the original adds the last band's score in place of
                // bands 2 to n-1, which only matters with three or more)
                self.v = bv[0];
                for &s in &bv[1..] {
                    self.v += if self.setup.emphasize_first_band { s / self.bands as f64 } else { s };
                }
            }
            self.calculations += 1;
            let p = Progress {
                calculation: self.calculations,
                variable: n,
                band: b,
                frequency_mhz: self.model.frequency,
                impedance: (rd.r as f64, rd.jx as f64).into(),
                swr: rd.swr,
                gain_dbi: rd.gain,
                fb_db: rd.fb,
                elevation_deg: rd.elv,
                current: rd.current,
                score: last.then_some(self.v),
                improved: last && self.v > self.vm,
                values: self.values(),
            };
            if !(self.progress)(&p) {
                self.abort = true;
            }
            if self.limit.is_some_and(|l| self.calculations >= l) {
                self.abort = true;
                self.limited = true;
            }
            if b == 0 {
                first = Some(rd);
            }
            if !last && self.abort {
                return Err(Stop::Cancelled);
            }
        }
        let rd = first.expect("at least one band");
        let trial = Trial {
            values: self.values(),
            score: self.v,
            impedance: (rd.r as f64, rd.jx as f64).into(),
            swr: rd.swr,
            gain_dbi: rd.gain,
            fb_db: rd.fb,
            elevation_deg: rd.elv,
            horizontal_pattern: rd.horizontal,
            vertical_pattern: rd.vertical,
        };
        let improved = self.vm < self.v;
        if improved {
            self.vm = self.v;
            self.ad = self.values();
            self.best = Some(trial.clone());
        }
        // `WriteMV`
        self.trials.push_front(trial);
        self.trials.truncate(TRIALS_KEPT);
        Ok(improved)
    }

    /// `CACal::Exec`: varies each independent variable in turn while the
    /// score rises, doubling the step as long as it keeps rising, and repeats
    /// the round until a whole round finds nothing better.
    fn exec(&mut self) -> Result<(), Stop> {
        self.model.set_band(0);
        for n in 0..self.vars.len() {
            self.set_para_linked(n);
        }
        let mut max_v = -f64::MAX;
        let mut init = false;
        loop {
            let mut found = false;
            for n in 0..self.vars.len() {
                if !self.independent(n) {
                    continue;
                }
                let mut dir = 1.0;
                self.am = max_v;
                let mut max_d = self.state[n].dc;
                let s = &mut self.state[n];
                if s.di > s.dio {
                    s.di = s.dio;
                }
                let mut pf = 0;
                let mut cont_n = 0;
                loop {
                    if init {
                        let s = &mut self.state[n];
                        let before = s.dc;
                        s.dc += s.di * dir;
                        let mut dc = s.dc;
                        // a sweep only turns at a bound, which a step that does
                        // not move the value (pitch 0) never reaches: the
                        // original loops on its memo forever, never calculating
                        // again, so it could not even be stopped
                        let stuck = self.setup.sweep_only && dc == before;
                        if self.clamp(n, &mut dc) || stuck {
                            // back to the best value, and the other way
                            let mut d = max_d;
                            self.clamp(n, &mut d);
                            self.state[n].dc = d;
                            if dir >= 1.0 {
                                dir = -1.0;
                                let s = &mut self.state[n];
                                if s.di > s.dio {
                                    s.di = s.dio;
                                }
                                cont_n = 0;
                                continue;
                            }
                            self.set_para_linked(n);
                            break;
                        }
                    }
                    self.set_para_linked(n);
                    if self.is_same_max() {
                        self.v = self.vm;
                    } else if let Some(v) = self.remembered() {
                        self.v = v;
                    } else if self.trial(n)? {
                        found = true;
                    }
                    if self.abort {
                        return Err(Stop::Cancelled);
                    }
                    if max_v < self.v {
                        max_v = self.v;
                        max_d = self.state[n].dc;
                    }
                    if self.setup.sweep_only {
                        init = true;
                        continue;
                    }
                    let s = &mut self.state[n];
                    if self.am < self.v {
                        // rising
                        self.am = self.v;
                        if init && pf == 0 {
                            pf = 1;
                        }
                        if pf == 1 {
                            if cont_n >= 8 {
                                s.di = s.dio * 32.0;
                            } else if s.di < (s.dio * 16.0) {
                                s.di *= 2.0;
                            }
                        } else if pf == 2 {
                            s.dc = max_d;
                            self.set_para_linked(n);
                            break;
                        }
                    } else if pf == 1 && s.di >= s.dio * 16.0 {
                        // overshot with a long step: back with a quarter of it
                        s.dc = max_d;
                        s.di /= 4.0;
                        cont_n = 0;
                        pf = 2;
                    } else if pf != 0 {
                        s.dc = max_d;
                        self.set_para_linked(n);
                        break;
                    } else if dir >= 1.0 {
                        // falling at once: try the other way
                        dir = -1.0;
                        s.dc = max_d;
                        if s.di > s.dio {
                            s.di = s.dio;
                        }
                        cont_n = 0;
                    } else {
                        s.dc = max_d;
                        self.set_para_linked(n);
                        break;
                    }
                    init = true;
                    cont_n += 1;
                }
            }
            if !found || self.setup.sweep_only {
                return Ok(());
            }
        }
    }
}

/// Optimises the antenna (`CACal::Execute`). `options` are those of a normal
/// calculation (its frequency is the starting frequency; the pattern
/// resolution comes from the setup). `progress` is called after every
/// calculation and stops the search by returning false; `limit` caps the
/// number of calculations. Either way the best antenna so far is returned.
pub fn optimize(
    project: &Project,
    options: &CalcOptions,
    setup: &OptimizerSetup,
    limit: Option<usize>,
    progress: impl FnMut(&Progress) -> bool,
) -> Result<OptimizeOutcome, OptimizeError> {
    let mut model = Model::new(project, options);
    let start_frequency = model.frequency;
    let (errors, warnings) = check(&mut model, setup);
    if !errors.is_empty() {
        return Err(OptimizeError::Setup(errors));
    }
    let Prepared { vars, state, ad } = init_para(&model, setup).map_err(OptimizeError::Setup)?;
    model.bands = if uses_bands(&model.antenna, setup) {
        setup.bands.clone()
    } else {
        let sources = model.antenna.sources.iter().take(BAND_SOURCES).cloned().collect();
        vec![Band { frequency_mhz: model.frequency, sources }]
    };
    let options = CalcOptions {
        pattern_resolution: if setup.coarse_pattern { PatternResolution::Coarse } else { PatternResolution::Fine },
        ..options.clone()
    };
    let mut search = Search {
        bands: model.bands.len(),
        model,
        vars,
        state,
        setup,
        options,
        trade: setup.weights.trade_off(),
        v: 1.0,
        am: -f64::MAX,
        vm: -f64::MAX,
        ad,
        trials: VecDeque::new(),
        best: None,
        calculations: 0,
        limit,
        abort: false,
        limited: false,
        progress,
    };
    let stop = match search.exec() {
        Ok(()) => StopReason::Converged,
        Err(Stop::Cancelled) if search.limited => StopReason::CalculationLimit,
        Err(Stop::Cancelled) => StopReason::Cancelled,
        Err(Stop::Failed(e)) => StopReason::Failed { message: e.to_string() },
    };
    let (project, frequency_mhz) = if search.best.is_some() {
        if search.bands > 1 {
            // (the original keeps the last band's frequency and sources)
            search.model.antenna.sources = project.antenna.sources.clone();
            search.model.frequency = start_frequency;
        }
        for i in 0..search.vars.len() {
            let d = search.ad[i];
            search.set_para(i, d);
        }
        search.model.update_wires();
        (search.model.project(), search.model.frequency)
    } else {
        (project.clone(), start_frequency)
    };
    Ok(OptimizeOutcome {
        project,
        frequency_mhz,
        stop,
        calculations: search.calculations,
        best: search.best,
        trials: search.trials.into(),
        warnings,
    })
}

/// Writes the values of a trial back (the trial list's 更新): the antenna
/// and the calculation frequency. `project` and `options` must be those the
/// optimisation started from: elements are numbered by where they lie, which
/// another trial may have changed. (MMANA analyses the antenna as it is now,
/// and gets the wrong elements then.)
pub fn apply_trial(
    project: &Project,
    options: &CalcOptions,
    setup: &OptimizerSetup,
    values: &[f64],
) -> Result<(Project, f64), OptimizeError> {
    let vars = &setup.variables;
    if values.len() != vars.len() {
        return Err(OptimizeError::ValueCount { expected: vars.len(), got: values.len() });
    }
    let mut model = Model::new(project, options);
    let missing: Vec<SetupIssue> = (0..vars.len())
        .filter(|&i| !model.exists(&vars[i].target))
        .map(|i| SetupIssue::new(IssueCode::MissingTarget, Some(i), None))
        .collect();
    if !missing.is_empty() {
        return Err(OptimizeError::Setup(missing));
    }
    for (v, &d) in vars.iter().zip(values) {
        model.set(&v.target, d);
    }
    model.update_wires();
    Ok((model.project(), model.frequency))
}

// --------------------------------------------------------- setup edits ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum ElementAxis {
    Width,
    Height,
    XWidth,
}

/// What "all elements" registers for elements that have two or more of
/// width, height and X width (MMANA asks once, at the first such element,
/// and keeps the answer for the rest).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ElementCombination {
    pub width: bool,
    pub height: bool,
    pub x_width: bool,
    /// Link the width to the height or the X width (in proportion).
    pub width_link: Option<ElementAxis>,
    pub height_link: Option<ElementAxis>,
    pub x_width_link: Option<ElementAxis>,
}

impl Default for ElementCombination {
    fn default() -> Self {
        Self { width: true, height: true, x_width: true, width_link: None, height_link: None, x_width_link: None }
    }
}

/// An edit of the variable table (the setup dialog's buttons and menus).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum SetupOp {
    /// Sets what variable `index` varies (`index` = count adds one) and its
    /// default pitch and bounds; an index past the last thing of its kind
    /// becomes the last one.
    SetTarget {
        index: usize,
        target: VariableTarget,
    },
    /// Inserts a copy of variable `index` before it (この行に挿入).
    Insert {
        index: usize,
    },
    /// Variables linked to it become independent.
    Delete {
        index: usize,
    },
    Clear,
    /// Registers every element's sizes and intervals (全エレメント), in the
    /// order MMANA finds best: radiator, reflectors, directors. Equal values on
    /// the same X are linked.
    #[serde(rename_all = "camelCase")]
    AllElements {
        /// Intervals (間隔表記) rather than positions.
        intervals: bool,
        /// The `…All` parts (全ての座標を同一比率で変更).
        scale_all: bool,
        #[serde(default)]
        combination: Option<ElementCombination>,
    },
    /// Adds the wire ends or loads with the same value as variable `index`,
    /// linked to it (自動連携登録).
    LinkSame {
        index: usize,
    },
    /// Links element variables of equal value (エレメントの連携設定).
    LinkElements,
}

/// Result of [`edit_setup`].
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SetupEdit {
    pub setup: OptimizerSetup,
    /// Whether each operation was applied (false: not allowed, nothing changed).
    pub applied: Vec<bool>,
    /// Value of what each variable varies now (`null` when it does not exist).
    pub values: Vec<Option<f64>>,
    /// For linked variables, the value the link gives.
    pub linked_values: Vec<Option<f64>>,
    /// The element table the element variables refer to.
    pub elements: Vec<ElementRow>,
    pub base: usize,
    /// What stops a run.
    pub errors: Vec<SetupIssue>,
    pub warnings: Vec<SetupIssue>,
}

/// Whether two loads are of the same type (`LDEF::TYPE`).
fn same_load_kind(a: &LoadKind, b: &LoadKind) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}

/// The "ask once" state of "all elements" (`WHFlag`, `WHHW`…, `WHHC`…).
struct Combination {
    asked: bool,
    width: bool,
    height: bool,
    x_width: bool,
    links: [Option<ElementAxis>; 3],
    answer: ElementCombination,
}

struct Editor {
    model: Model,
    setup: OptimizerSetup,
}

impl Editor {
    fn vars(&self) -> &Vec<Variable> {
        &self.setup.variables
    }

    fn value(&self, n: usize) -> f64 {
        self.model.value(&self.setup.variables[n].target)
    }

    fn pitch(&self, d: f64, dm: f64) -> f64 {
        let p = if self.setup.absolute_pitch { get_normal(d / dm) } else { 100.0 / dm };
        if p == 0.0 { 0.001 } else { p }
    }

    /// `AddAVAL`: appends a variable; `None` when the table is full.
    fn add(&mut self, target: VariableTarget, link: Link, d: f64, dm: f64) -> Option<usize> {
        if self.vars().len() >= MAX_VARIABLES {
            return None;
        }
        let min = match target {
            VariableTarget::Wire { part, .. } if part != WirePart::Radius => -2000.0,
            _ => 0.0,
        };
        let pitch = self.pitch(d, dm);
        self.setup.variables.push(Variable { target, link, pitch, min, max: 2000.0, min_from: None, max_from: None });
        Some(self.vars().len() - 1)
    }

    fn apply(&mut self, op: &SetupOp) -> bool {
        let len = self.vars().len();
        match *op {
            SetupOp::SetTarget { index, target } => {
                if index > len || (index == len && len >= MAX_VARIABLES) {
                    return false;
                }
                let mut v = self.setup.variables.get(index).cloned().unwrap_or_else(|| Variable::new(target));
                v.target = target;
                if !adjust(&self.model, self.setup.absolute_pitch, &mut v) {
                    return false;
                }
                if index == len {
                    self.setup.variables.push(v);
                } else {
                    self.setup.variables[index] = v;
                }
            }
            SetupOp::Insert { index } => {
                if index >= len || len >= MAX_VARIABLES {
                    return false;
                }
                let shift = |k: usize| if k >= index { k + 1 } else { k };
                for v in &mut self.setup.variables {
                    if let Some(k) = v.link.followed() {
                        v.link = v.link.with_variable(shift(k));
                    }
                    v.min_from = v.min_from.map(shift);
                    v.max_from = v.max_from.map(shift);
                }
                let copy = self.setup.variables[index].clone();
                self.setup.variables.insert(index, copy);
            }
            SetupOp::Delete { index } => {
                if index >= len {
                    return false;
                }
                let shift = |k: usize| {
                    if k > index {
                        Some(k - 1)
                    } else if k == index {
                        None
                    } else {
                        Some(k)
                    }
                };
                for v in &mut self.setup.variables {
                    if let Some(k) = v.link.followed() {
                        v.link = shift(k).map_or(Link::Independent, |k| v.link.with_variable(k));
                    }
                    v.min_from = v.min_from.and_then(shift);
                    v.max_from = v.max_from.and_then(shift);
                }
                self.setup.variables.remove(index);
            }
            SetupOp::Clear => self.setup.variables.clear(),
            SetupOp::AllElements { intervals, scale_all, combination } => {
                self.all_elements(intervals, scale_all, combination.unwrap_or_default());
            }
            SetupOp::LinkSame { index } => {
                if index >= len || !self.model.exists(&self.vars()[index].target) {
                    return false;
                }
                match self.vars()[index].target {
                    VariableTarget::Wire { .. } => self.link_same_wires(index),
                    VariableTarget::Load { .. } => self.link_same_loads(index),
                    _ => return false,
                }
            }
            SetupOp::LinkElements => self.link_elements(),
        }
        true
    }

    /// `AllEleClick`
    fn all_elements(&mut self, intervals: bool, scale_all: bool, answer: ElementCombination) {
        self.setup.variables.clear();
        let gmax = self.model.list.len() as isize;
        if gmax == 0 {
            return;
        }
        let interval = |e: &Self, i: isize| e.model.list.element_interval(i as usize);
        let position = |e: &Self, i: isize| e.model.list.element_position(i as usize);
        let base0 = self.model.list.base() as isize;
        // the last element at the base element's X
        let mut base = base0;
        while base < gmax && interval(self, base) == 0.0 {
            base += 1;
        }
        if base == gmax || interval(self, base) != 0.0 {
            base -= 1;
        }
        let mut c = Combination { asked: false, width: true, height: true, x_width: true, links: [None; 3], answer };
        // the radiator
        let mut i = base;
        while i >= base0 {
            self.add_element(i as usize, scale_all, &mut c);
            i -= 1;
        }
        // reflectors and the intervals or positions
        while i >= 0 {
            self.add_element(i as usize, scale_all, &mut c);
            if intervals {
                let d = interval(self, i);
                if d != 0.0 {
                    self.add(Self::element(i, ElementPart::Interval), Link::Independent, d, 100.0);
                }
            } else {
                let d = position(self, i);
                if i != 0 && d != 0.0 {
                    self.add(Self::element(i, ElementPart::Position), Link::Independent, d, 500.0);
                }
            }
            i -= 1;
        }
        if !intervals && base < gmax - 1 {
            for i in (base0..=base).rev() {
                let d = position(self, i);
                if d != 0.0 {
                    self.add(Self::element(i, ElementPart::Position), Link::Independent, d, 500.0);
                }
            }
        }
        // directors
        let last_x = position(self, gmax - 1);
        for i in base + 1..gmax {
            self.add_element(i as usize, scale_all, &mut c);
            if intervals {
                let d = interval(self, i);
                if d != 0.0 {
                    self.add(Self::element(i, ElementPart::Interval), Link::Independent, d, 100.0);
                }
            } else {
                let d = position(self, i);
                if i < gmax - 1 && d != last_x {
                    self.add(Self::element(i, ElementPart::Position), Link::Independent, d, 500.0);
                }
            }
        }
        self.link_same_x();
    }

    fn element(i: isize, part: ElementPart) -> VariableTarget {
        VariableTarget::Element { element: i as usize, part }
    }

    /// `AddEle`
    fn add_element(&mut self, n: usize, scale_all: bool, c: &mut Combination) {
        let l = &self.model.list;
        let part = |edge: ElementPart, all: ElementPart| if scale_all { all } else { edge };
        if !l.element_x_width(n) {
            let x = l.element_perimeter(n);
            if x != 0.0 {
                let target = Self::element(n as isize, part(ElementPart::Perimeter, ElementPart::PerimeterAll));
                self.add(target, Link::Independent, x, 500.0);
            }
            return;
        }
        let (h, v, x) = (l.element_width(n), l.element_height(n), l.element_perimeter(n));
        if !c.asked {
            (c.width, c.height, c.x_width) = (h != 0.0, v != 0.0, x != 0.0);
            if (c.width && c.height) || (c.width && c.x_width) || (c.height && c.x_width) {
                // `TACalEleBox`: what to register and link
                let a = c.answer;
                c.width &= a.width;
                c.height &= a.height;
                c.x_width &= a.x_width;
                c.links = [a.width_link, a.height_link, a.x_width_link];
                c.asked = true;
            }
        }
        let add = |e: &mut Self, on: bool, d: f64, edge, all| {
            if d != 0.0 && on {
                e.add(Self::element(n as isize, part(edge, all)), Link::Independent, d, 500.0)
            } else {
                None
            }
        };
        let hp = add(self, c.width, h, ElementPart::Width, ElementPart::WidthAll);
        let vp = add(self, c.height, v, ElementPart::Height, ElementPart::HeightAll);
        let xp = add(self, c.x_width, x, ElementPart::Perimeter, ElementPart::PerimeterAll);
        use ElementAxis::*;
        let [hc, vc, xc] = c.links;
        // `SetEleWith`: the same value, or in proportion
        let link = |e: &mut Self, n: Option<usize>, w: Option<usize>, a: f64, b: f64| {
            if let (Some(n), Some(w)) = (n, w) {
                e.setup.variables[n].link = if round_up_str(a) == round_up_str(b) {
                    Link::Same { variable: w }
                } else {
                    Link::Scaled { variable: w, factor: a / b }
                };
            }
        };
        if hc == Some(Height) {
            link(self, hp, vp, h, v);
        }
        if hc == Some(XWidth) {
            link(self, hp, xp, h, x);
        }
        if vc == Some(Width) {
            link(self, vp, hp, v, h);
        }
        if vc == Some(XWidth) {
            link(self, vp, xp, v, x);
        }
        if xc == Some(Width) {
            link(self, xp, hp, x, h);
        }
        if xc == Some(Height) {
            link(self, xp, vp, x, v);
        }
    }

    /// `AdjWithEle`: links equal sizes and positions of elements on the same X.
    fn link_same_x(&mut self) {
        let len = self.vars().len();
        for i in 0..len {
            let VariableTarget::Element { element, part } = self.vars()[i].target else { continue };
            if self.vars()[i].link != Link::Independent || part == ElementPart::Interval || part == ElementPart::Radius
            {
                continue;
            }
            let x = round_up_str(self.model.list.element_position(element));
            let d = round_up_str(self.value(i));
            for j in i + 1..len {
                let VariableTarget::Element { element: e2, part: p2 } = self.vars()[j].target else { continue };
                if p2 == part
                    && x == round_up_str(self.model.list.element_position(e2))
                    && d == round_up_str(self.value(j))
                {
                    self.setup.variables[j].link = Link::Same { variable: i };
                }
            }
        }
    }

    /// `K34Click`: links element variables of equal value (width and height
    /// count as one kind).
    fn link_elements(&mut self) {
        let len = self.vars().len();
        for i in 0..len {
            let VariableTarget::Element { part, .. } = self.vars()[i].target else { continue };
            if self.vars()[i].link != Link::Independent || !self.model.exists(&self.vars()[i].target) {
                continue;
            }
            let d = round_up_str(self.value(i));
            for j in i + 1..len {
                let w = &self.vars()[j];
                let VariableTarget::Element { part: p2, .. } = w.target else { continue };
                if w.link != Link::Independent || !self.model.exists(&w.target) {
                    continue;
                }
                let sides = [ElementPart::Width, ElementPart::Height];
                let kin = if sides.contains(&part) { sides.contains(&p2) } else { part == p2 };
                if kin && d == round_up_str(self.value(j)) {
                    self.setup.variables[j].link = Link::Same { variable: i };
                }
            }
        }
    }

    /// `CheckSameWireChen`: a wire variable already moves point `p`.
    fn moves_point(&self, p: crate::Point3) -> bool {
        let wires = &self.model.antenna.wires;
        self.vars().iter().any(|v| match v.target {
            VariableTarget::Wire { wire, part } => wires.get(wire).is_some_and(|w| {
                let q = if matches!(part, WirePart::Y1 | WirePart::Z1 | WirePart::X1) { w.start } else { w.end };
                q == p
            }),
            _ => false,
        })
    }

    /// `AddSameWireChen`: the wire ends (or radii) with variable `n`'s value
    /// or its negation, linked to it.
    fn link_same_wires(&mut self, n: usize) {
        let VariableTarget::Wire { wire: pos, part } = self.vars()[n].target else { return };
        let wires = self.model.antenna.wires.clone();
        let d = match part {
            WirePart::X1 => wires[pos].start.x,
            WirePart::X2 => wires[pos].end.x,
            _ => self.value(n),
        };
        let d = round_up(d, 100000.0);
        let try_add = |e: &mut Self, i: usize, p: WirePart, point: crate::Point3, v: f64| {
            if (i != pos || part != p) && !e.moves_point(point) {
                let dd = round_up(v, 100000.0);
                let link = if dd == d {
                    Link::Same { variable: n }
                } else if dd == -d {
                    Link::Negated { variable: n }
                } else {
                    return;
                };
                e.add(VariableTarget::Wire { wire: i, part: p }, link, d, 100.0);
            }
        };
        for (i, w) in wires.iter().enumerate().take(self.model.list.wire_count()) {
            match part {
                WirePart::Radius => {
                    if i != pos && round_up(w.radius, 100000.0) == d {
                        self.add(VariableTarget::Wire { wire: i, part }, Link::Same { variable: n }, d, 100.0);
                    }
                }
                WirePart::X1 | WirePart::X2 => {
                    try_add(self, i, WirePart::X1, w.start, w.start.x);
                    try_add(self, i, WirePart::X2, w.end, w.end.x);
                }
                _ => {
                    try_add(self, i, WirePart::Y1, w.start, w.start.y);
                    try_add(self, i, WirePart::Z1, w.start, w.start.z);
                    try_add(self, i, WirePart::Y2, w.end, w.end.y);
                    try_add(self, i, WirePart::Z2, w.end, w.end.z);
                }
            }
        }
    }

    /// `AddSameLoadChen`: the loads of the same type and value, linked.
    fn link_same_loads(&mut self, n: usize) {
        let VariableTarget::Load { load: pos, part } = self.vars()[n].target else { return };
        let loads = self.model.antenna.loads.clone();
        let d = round_up(self.value(n), 100000.0);
        for (i, l) in loads.iter().enumerate() {
            if i == pos || !same_load_kind(&l.kind, &loads[pos].kind) {
                continue;
            }
            let (a, b) = load_values(&l.kind);
            if round_up(if part == LoadPart::Secondary { b } else { a }, 100000.0) == d {
                self.add(VariableTarget::Load { load: i, part }, Link::Same { variable: n }, d, 100.0);
            }
        }
    }
}

/// `AdjSetup`: the default pitch and bounds of a variable; a position past
/// the last thing of its kind becomes the last one. False when there is
/// nothing of that kind.
fn adjust(model: &Model, absolute: bool, v: &mut Variable) -> bool {
    use VariableTarget as T;
    let count = model.count(&v.target);
    if count == 0 {
        return false;
    }
    match &mut v.target {
        T::Wire { wire: i, .. }
        | T::PolarWire { wire: i, .. }
        | T::Element { element: i, .. }
        | T::Load { load: i, .. }
        | T::Source { source: i, .. } => *i = (*i).min(count - 1),
        T::Height | T::Frequency | T::Stack { .. } => {}
    }
    let value = model.value(&v.target);
    let normal = |div: f64, otherwise: f64| if absolute { get_normal(value / div) } else { otherwise };
    let nonzero = |p: f64| if p == 0.0 { 0.1 } else { p };
    (v.min, v.max, v.pitch) = match v.target {
        T::Wire { part: WirePart::Radius, .. } => (0.0, 0.5, nonzero(normal(200.0, 0.5))),
        T::Wire { .. } => (-2000.0, 2000.0, nonzero(normal(200.0, 0.5))),
        T::Element { part: ElementPart::Radius, .. } => (0.0, 0.5, nonzero(normal(200.0, 0.5))),
        T::Element { part: ElementPart::Position, .. } => (0.0, 2000.0, normal(200.0, 0.5)),
        T::Element { .. } => (0.0, 2000.0, nonzero(normal(200.0, 0.5))),
        T::Load { .. } => (0.0, 10000.0, nonzero(normal(200.0, 0.5))),
        T::Height => (0.0, 200.0, if absolute { 1.0 } else { 10.0 }),
        T::Frequency => (1.5, 1300.0, nonzero(normal(100.0, 1.0))),
        T::PolarWire { quantity: PolarQuantity::Length, .. } => (0.0, 2000.0, nonzero(normal(200.0, 1.0))),
        T::PolarWire { .. } => (-360.0, 360.0, nonzero(normal(200.0, 1.0))),
        T::Source { part: SourcePart::Voltage, .. } => (0.0, 1.0, 0.01),
        T::Source { .. } => (-360.1, 360.1, 1.0),
        T::Stack { .. } => (0.0, 2000.0, if absolute { 0.01 } else { 1.0 }),
    };
    true
}

/// Opens the setup dialog on the antenna, applies `ops` in order and
/// describes the result: current values, the element table and what would
/// stop a run.
pub fn edit_setup(project: &Project, options: &CalcOptions, setup: &OptimizerSetup, ops: &[SetupOp]) -> SetupEdit {
    let mut editor = Editor { model: Model::new(project, options), setup: setup.clone() };
    let applied = ops.iter().map(|op| editor.apply(op)).collect();
    let Editor { model, setup } = editor;
    let vars = &setup.variables;
    let values: Vec<Option<f64>> =
        vars.iter().map(|v| model.exists(&v.target).then(|| model.value(&v.target))).collect();
    let linked_values = (0..vars.len())
        .map(|i| {
            vars[i].link.followed()?;
            let k = last_in_chain(vars, i).ok()?;
            values[k].map(|d| vars[i].link.apply(d))
        })
        .collect();
    let (errors, warnings) = issues(&model, &setup);
    SetupEdit {
        elements: model.list.rows(),
        base: model.list.base(),
        values,
        linked_values,
        applied,
        errors,
        warnings,
        setup,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::maa;

    fn sample(name: &str) -> Project {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/maa/");
        maa::parse_bytes(&std::fs::read(format!("{path}{name}")).unwrap()).project
    }

    fn dipole_length() -> OptimizerSetup {
        let mut setup = OptimizerSetup::default();
        let ops = [
            SetupOp::SetTarget { index: 0, target: VariableTarget::Wire { wire: 0, part: WirePart::Y2 } },
            SetupOp::LinkSame { index: 0 },
        ];
        let edit = edit_setup(&sample("DP20.MAA"), &CalcOptions::default(), &setup, &ops);
        assert_eq!(edit.applied, vec![true, true]);
        setup.variables = edit.setup.variables;
        setup.weights = Weights { gain: 0, fb: 0, jx: 0, swr: 100, elevation: 0, matching: 0, current: 0 };
        setup
    }

    /// The score of the first trial (the starting antenna).
    fn first_score(project: &Project, options: &CalcOptions, setup: &OptimizerSetup) -> f64 {
        let mut score = None;
        optimize(project, options, setup, None, |p| {
            score = p.score;
            score.is_none()
        })
        .unwrap();
        score.unwrap()
    }

    #[test]
    fn every_band_counts() {
        let project = sample("DP20.MAA");
        let mut setup = dipole_length();
        let freqs = [13.9, 14.1, 14.35];
        let alone: Vec<f64> = freqs
            .iter()
            .map(|&f| first_score(&project, &CalcOptions { frequency_mhz: Some(f), ..Default::default() }, &setup))
            .collect();
        let source = project.antenna.sources.clone();
        setup.bands = freqs.iter().map(|&f| Band { frequency_mhz: f, sources: source.clone() }).collect();
        setup.emphasize_first_band = false;
        let all = first_score(&project, &CalcOptions::default(), &setup);
        assert_eq!(all, alone[0] + alone[1] + alone[2]);
        setup.emphasize_first_band = true;
        let first = first_score(&project, &CalcOptions::default(), &setup);
        assert_eq!(first, alone[0] + alone[1] / 3.0 + alone[2] / 3.0);
    }

    #[test]
    fn bands_end_with_the_antennas_own_sources() {
        let project = sample("DP20.MAA");
        let mut setup = dipole_length();
        let source = |phase_deg| Source { pulse: "w1c".into(), phase_deg, voltage: 1.0 };
        setup.bands = vec![
            Band { frequency_mhz: 13.9, sources: vec![source(10.0)] },
            Band { frequency_mhz: 14.3, sources: vec![source(20.0)] },
        ];
        let out = optimize(&project, &CalcOptions::default(), &setup, Some(30), |_| true).unwrap();
        assert_eq!(out.project.antenna.sources, project.antenna.sources);
        assert_eq!(out.frequency_mhz, project.antenna.frequency_mhz);
        assert_ne!(out.project.antenna.wires, project.antenna.wires);
    }

    #[test]
    fn cancelling_keeps_the_best_so_far() {
        let project = sample("DP20.MAA");
        let setup = dipole_length();
        let mut seen = 0;
        let out = optimize(&project, &CalcOptions::default(), &setup, None, |_| {
            seen += 1;
            seen < 3
        })
        .unwrap();
        assert_eq!(out.stop, StopReason::Cancelled);
        assert_eq!(out.calculations, 3);
        let best = out.best.unwrap();
        assert_eq!(out.trials.len(), 3);
        assert!(out.trials.iter().all(|t| t.score <= best.score));
        assert_eq!(out.project.antenna.wires[0].end.y, best.values[0]);
        assert_eq!(out.project.antenna.wires[0].start.y, best.values[1]);
    }

    #[test]
    fn applying_a_trial_sets_its_values() {
        let project = sample("DP20.MAA");
        let setup = dipole_length();
        let out = optimize(&project, &CalcOptions::default(), &setup, Some(20), |_| true).unwrap();
        let t = out.trials.last().unwrap();
        let (applied, f) = apply_trial(&project, &CalcOptions::default(), &setup, &t.values).unwrap();
        assert_eq!(applied.antenna.wires[0].end.y, t.values[0]);
        assert_eq!(applied.antenna.wires[0].start.y, t.values[1]);
        assert_eq!(f, project.antenna.frequency_mhz);
        assert_eq!(
            apply_trial(&project, &CalcOptions::default(), &setup, &[1.0]),
            Err(OptimizeError::ValueCount { expected: 2, got: 1 })
        );
    }

    #[test]
    fn a_failed_calculation_keeps_the_best_so_far() {
        let mut project = sample("DP20.MAA");
        let mut setup = dipole_length();
        // a radius that runs negative selects a combined wire that does not exist
        setup.variables.push(Variable::new(VariableTarget::Wire { wire: 0, part: WirePart::Radius }));
        let v = setup.variables.last_mut().unwrap();
        // the maximum turns the first step round, below zero
        (v.pitch, v.min, v.max) = (0.001, -1.0, 0.0006);
        project.antenna.wires[0].radius = 0.0005;
        let out = optimize(&project, &CalcOptions::default(), &setup, None, |_| true).unwrap();
        assert!(matches!(out.stop, StopReason::Failed { .. }), "{:?}", out.stop);
        assert!(out.best.is_some());
        assert!(out.project.antenna.wires[0].radius > 0.0);
    }

    #[test]
    fn setup_edits_renumber_links_and_bounds() {
        let project = sample("5EL20.MAA");
        let opts = CalcOptions::default();
        let ele = |element, part| VariableTarget::Element { element, part };
        let mut setup = OptimizerSetup::default();
        for (i, e) in [0, 1, 2].into_iter().enumerate() {
            let edit = edit_setup(
                &project,
                &opts,
                &setup,
                &[SetupOp::SetTarget { index: i, target: ele(e, ElementPart::Width) }],
            );
            setup = edit.setup;
        }
        setup.variables[2].link = Link::Scaled { variable: 1, factor: 1.1 };
        setup.variables[0].max_from = Some(2);
        let edit = edit_setup(&project, &opts, &setup, &[SetupOp::Insert { index: 1 }]);
        assert_eq!(edit.setup.variables[3].link, Link::Scaled { variable: 2, factor: 1.1 });
        assert_eq!(edit.setup.variables[0].max_from, Some(3));
        // the inserted copy duplicates variable 1
        assert_eq!(
            edit.errors.iter().map(|e| e.code).collect::<Vec<_>>(),
            vec![IssueCode::Duplicate, IssueCode::Duplicate]
        );
        assert_eq!(edit.linked_values[3], edit.values[2].map(|d| d * 1.1));
        let edit = edit_setup(&project, &opts, &edit.setup, &[SetupOp::Delete { index: 2 }]);
        assert_eq!(edit.setup.variables.len(), 3);
        assert_eq!(edit.setup.variables[2].link, Link::Independent);
        assert_eq!(edit.setup.variables[0].max_from, Some(2));
        assert!(edit.errors.is_empty(), "{:?}", edit.errors);
        assert!(edit.values.iter().all(Option::is_some));
    }

    #[test]
    fn set_target_defaults_like_the_original() {
        let project = sample("3EL20.MAA");
        let opts = CalcOptions::default();
        let setup = OptimizerSetup::default();
        let target = VariableTarget::Element { element: 9, part: ElementPart::Width };
        let edit = edit_setup(&project, &opts, &setup, &[SetupOp::SetTarget { index: 0, target }]);
        let v = &edit.setup.variables[0];
        // the last element; pitch = GetNormal(10 / 200)
        assert_eq!(v.target, VariableTarget::Element { element: 2, part: ElementPart::Width });
        assert_eq!((v.pitch, v.min, v.max), (0.05, 0.0, 2000.0));
        let percent = OptimizerSetup { absolute_pitch: false, ..setup };
        let edit =
            edit_setup(&project, &opts, &percent, &[SetupOp::SetTarget { index: 0, target: VariableTarget::Height }]);
        assert_eq!(edit.setup.variables[0].pitch, 10.0);
        let edit =
            edit_setup(&project, &opts, &percent, &[SetupOp::SetTarget { index: 1, target: VariableTarget::Height }]);
        assert_eq!(edit.applied, vec![false]);
    }

    #[test]
    fn source_variables_need_the_source_in_every_band() {
        let project = sample("HB9CV.MAA");
        let mut setup = OptimizerSetup::default();
        let mut v = Variable::new(VariableTarget::Source { source: 1, part: SourcePart::Phase });
        (v.pitch, v.min, v.max) = (1.0, -360.1, 360.1);
        setup.variables.push(v);
        let one = |f| Band { frequency_mhz: f, sources: vec![project.antenna.sources[0].clone()] };
        setup.bands = vec![one(14.0), Band { frequency_mhz: 14.2, sources: project.antenna.sources.clone() }];
        let err = optimize(&project, &CalcOptions::default(), &setup, Some(5), |_| true).unwrap_err();
        let OptimizeError::Setup(issues) = err else { panic!("{err:?}") };
        assert_eq!(
            issues.iter().map(|i| (i.code, i.other)).collect::<Vec<_>>(),
            vec![(IssueCode::BandMissingSource, Some(0))]
        );
        // with the source in every band it runs
        setup.bands[0] = Band { frequency_mhz: 14.0, sources: project.antenna.sources.clone() };
        assert!(optimize(&project, &CalcOptions::default(), &setup, Some(5), |_| true).is_ok());
    }

    /// Trials are values for the antenna the search started from: elements are
    /// numbered by where they were then, which another trial may change.
    #[test]
    fn trials_apply_to_the_starting_antenna() {
        let project = sample("2EL20.MAA");
        let opts = CalcOptions::default();
        let mut setup = OptimizerSetup { sweep_only: true, ..Default::default() };
        let mut v = Variable::new(VariableTarget::Element { element: 1, part: ElementPart::Interval });
        (v.pitch, v.min, v.max) = (1.0, -5.0, 5.0);
        setup.variables.push(v);
        let out = optimize(&project, &opts, &setup, None, |_| true).unwrap();
        let trial = |d: f64| out.trials.iter().find(|t| (t.values[0] - d).abs() < 1e-9).unwrap().values.clone();
        let behind = trial(2.67 - 7.0);
        let back = trial(2.67);
        let (moved, _) = apply_trial(&project, &opts, &setup, &behind).unwrap();
        // the parasitic element now lies behind the driven one: from the moved
        // antenna, element 1 is the other element
        let (wrong, _) = apply_trial(&moved, &opts, &setup, &back).unwrap();
        let (again, _) = apply_trial(&project, &opts, &setup, &back).unwrap();
        let close = |a: &Project| {
            a.antenna.wires.iter().zip(&project.antenna.wires).all(|(w, o)| {
                let d = [w.start.x - o.start.x, w.start.y - o.start.y, w.end.x - o.end.x, w.end.y - o.end.y];
                d.iter().all(|d| d.abs() < 1e-12)
            })
        };
        assert!(!close(&wrong));
        assert!(close(&again));
    }

    #[test]
    fn a_sweep_with_no_step_ends() {
        let project = sample("DP20.MAA");
        let mut setup = OptimizerSetup { sweep_only: true, ..Default::default() };
        let mut v = Variable::new(VariableTarget::Height);
        (v.pitch, v.min, v.max) = (0.0, 0.0, 200.0);
        setup.variables.push(v);
        let out = optimize(&project, &CalcOptions::default(), &setup, Some(2), |_| true).unwrap();
        assert_eq!(out.stop, StopReason::Converged);
        assert_eq!(out.calculations, 1);
        assert_eq!(out.project.environment.height, project.environment.height);
        // a step too small to move the value is no step either
        setup.variables[0].pitch = 1e-300;
        let out = optimize(&project, &CalcOptions::default(), &setup, Some(2), |_| true).unwrap();
        assert_eq!(out.calculations, 1);
    }

    #[test]
    fn get_normal_and_round_up_str_match_the_original() {
        assert_eq!(get_normal(0.0532), 0.05);
        assert_eq!(get_normal(0.0732), 0.1);
        assert_eq!(get_normal(3.7), 3.0);
        assert_eq!(get_normal(-0.004), -0.004);
        assert_eq!(round_up_str(1.23456789), 1.234568);
        assert_eq!(round_up_str(-0.0000004), -0.0);
    }
}
