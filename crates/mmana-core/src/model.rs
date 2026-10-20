//! Public, serialisable description of an antenna and its environment.
//!
//! These types are what the API and the UI exchange. They are independent of
//! the engine's internal (MININEC) representation, which is built from them by
//! [`crate::prepare`].

use serde::{Deserialize, Serialize};

#[cfg(feature = "openapi")]
use utoipa::ToSchema;

/// A point in metres.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct Point3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Point3 {
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}

/// How a wire is divided into segments (MMANA's "SEG" column).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Segmentation {
    /// Fixed number of equal segments (SEG > 0).
    Fixed { count: i32 },
    /// Automatic equal division using the antenna's DM2 (SEG = 0).
    Auto,
    /// Automatic tapering at both ends using DM1/DM2 (SEG = -1).
    Taper,
    /// Tapering at the start only (SEG = -2).
    TaperStart,
    /// Tapering at the end only (SEG = -3).
    TaperEnd,
    /// Tapering with explicit DM1 and optionally DM2 ("dm1,dm2" in the SEG column).
    TaperCustom { dm1: i32, dm2: Option<i32> },
}

impl Segmentation {
    /// MMANA's packed integer encoding of the SEG column.
    pub fn to_code(self) -> i32 {
        match self {
            Segmentation::Fixed { count } => count,
            Segmentation::Auto => 0,
            Segmentation::Taper => -1,
            Segmentation::TaperStart => -2,
            Segmentation::TaperEnd => -3,
            Segmentation::TaperCustom { dm1, dm2: None } => -dm1.abs(),
            Segmentation::TaperCustom { dm1, dm2: Some(dm2) } => {
                (0x8000_0000u32 | (dm1.unsigned_abs() & 0x7fff) | ((dm2.unsigned_abs() & 0x7fff) << 16)) as i32
            }
        }
    }

    pub fn from_code(code: i32) -> Self {
        match code {
            c if c > 0 => Segmentation::Fixed { count: c },
            0 => Segmentation::Auto,
            -1 => Segmentation::Taper,
            -2 => Segmentation::TaperStart,
            -3 => Segmentation::TaperEnd,
            c if c < -10000 => Segmentation::TaperCustom { dm1: c & 0x7fff, dm2: Some((c >> 16) & 0x7fff) },
            c => Segmentation::TaperCustom { dm1: -c, dm2: None },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Wire {
    pub start: Point3,
    pub end: Point3,
    /// Wire radius in metres. A negative value refers to a
    /// [`CombinedWire`] whose `radiusKey` equals it; zero disables the wire.
    pub radius: f64,
    pub segmentation: Segmentation,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

/// Voltage source. `pulse` uses MMANA's notation: `w1c` (centre of wire 1),
/// `w2b` / `w2e` (begin / end), optional offset `w2b3`, or a raw pulse number.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub pulse: String,
    pub phase_deg: f64,
    pub voltage: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LoadKind {
    /// L (µH), C (pF) and Q. L and C both non-zero form a parallel resonator.
    #[serde(rename_all = "camelCase")]
    Lc { inductance_uh: f64, capacitance_pf: f64, q: f64 },
    /// Fixed impedance R + jX (Ω).
    Impedance { r: f64, x: f64 },
    /// S-domain rational function; `coefficients[i] = [numerator, denominator]` of s^i.
    SDomain { coefficients: Vec<[f64; 2]> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Load {
    pub pulse: String,
    #[serde(flatten)]
    pub kind: LoadKind,
}

/// Parameters for automatic segmentation (MMANA "自動分割").
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct AutoSegmentation {
    /// Finest segment length is λ/dm1 (used at wire ends when tapering).
    pub dm1: i32,
    /// Coarsest segment length is λ/dm2.
    pub dm2: i32,
    /// Taper growth factor (1 < sc <= 3).
    pub sc: f64,
    /// Number of finest segments at each tapered end.
    pub ec: i32,
}

impl Default for AutoSegmentation {
    fn default() -> Self {
        Self { dm1: 400, dm2: 40, sc: 2.0, ec: 1 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum StackDirection {
    /// Centred on the original height (上下).
    #[default]
    Centered,
    /// Copies stacked upwards (上向き).
    Up,
    /// Copies stacked downwards (下向き).
    Down,
}

/// Copies the whole antenna side by side (`horizontal`, along Y) and on top of
/// each other (`vertical`, along Z). 1 × 1 means no stacking.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Stack {
    pub horizontal: i32,
    pub vertical: i32,
    pub horizontal_spacing: f64,
    pub vertical_spacing: f64,
    pub direction: StackDirection,
}

impl Stack {
    pub fn none(spacing: f64) -> Self {
        Self {
            horizontal: 1,
            vertical: 1,
            horizontal_spacing: spacing,
            vertical_spacing: spacing,
            direction: StackDirection::Centered,
        }
    }

    pub fn is_stacked(&self) -> bool {
        self.horizontal > 1 || self.vertical > 1
    }
}

/// One section of a [`CombinedWire`]: `length` metres of tube with `radius`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct CombinedSection {
    pub length: f64,
    pub radius: f64,
}

/// Telescoping element made of tube sections of different radii
/// (MMANA "ワイヤ組み合わせ"). Wires reference it through a negative radius.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CombinedWire {
    /// The negative radius value wires use to select this definition.
    pub radius_key: f64,
    /// Bit 0: 0 = sections expand from the wire centre, 1 = from the start.
    /// Bit 1: keep tapering inside each section.
    pub mode: i32,
    pub sections: Vec<CombinedSection>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Antenna {
    pub name: String,
    /// Design frequency in MHz (also the default calculation frequency).
    pub frequency_mhz: f64,
    pub wires: Vec<Wire>,
    pub sources: Vec<Source>,
    /// Kept for the editor (MMANA's "auto voltage" checkbox); not used by the engine.
    pub auto_voltage: bool,
    pub loads: Vec<Load>,
    pub loads_enabled: bool,
    pub auto_segmentation: AutoSegmentation,
    pub stack: Stack,
    #[serde(default)]
    pub combined_wires: Vec<CombinedWire>,
    #[serde(default)]
    pub comment: String,
}

impl Default for Antenna {
    fn default() -> Self {
        let frequency_mhz = 14.05;
        Self {
            name: String::new(),
            frequency_mhz,
            wires: Vec::new(),
            sources: Vec::new(),
            auto_voltage: true,
            loads: Vec::new(),
            loads_enabled: true,
            auto_segmentation: AutoSegmentation::default(),
            stack: Stack::none(SPEED_OF_LIGHT_MMANA / frequency_mhz),
            combined_wires: Vec::new(),
            comment: String::new(),
        }
    }
}

/// The value MMANA uses for c in "λ = 299.8 / f".
pub const SPEED_OF_LIGHT_MMANA: f64 = 299.8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum GroundKind {
    #[default]
    FreeSpace,
    Perfect,
    /// Perfect ground for the currents, real (lossy) ground for the far field.
    Real,
}

impl GroundKind {
    pub fn code(self) -> i32 {
        self as i32
    }

    pub fn from_code(code: i32) -> Self {
        match code {
            1 => GroundKind::Perfect,
            2 => GroundKind::Real,
            _ => GroundKind::FreeSpace,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum WireMaterial {
    #[default]
    Lossless,
    Copper,
    CopperPipe,
    Aluminum,
    AluminumPipe,
    Iron,
    IronPipe,
}

impl WireMaterial {
    pub fn code(self) -> i32 {
        self as i32
    }

    pub fn from_code(code: i32) -> Self {
        match code {
            1 => WireMaterial::Copper,
            2 => WireMaterial::CopperPipe,
            3 => WireMaterial::Aluminum,
            4 => WireMaterial::AluminumPipe,
            5 => WireMaterial::Iron,
            6 => WireMaterial::IronPipe,
            _ => WireMaterial::Lossless,
        }
    }
}

/// One ground medium of a real ground.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Medium {
    pub permittivity: f64,
    /// Conductivity in S/m.
    pub conductivity: f64,
    /// X (linear) or R (circular) coordinate where the next medium starts.
    pub boundary: Option<f64>,
    /// Height of this medium's surface.
    pub height: Option<f64>,
}

impl Default for Medium {
    fn default() -> Self {
        Self { permittivity: 5.0, conductivity: 1.0e-3, boundary: None, height: None }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct GroundScreen {
    pub radials: i32,
    pub radial_radius: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct RealGround {
    pub media: Vec<Medium>,
    /// Radial ground screen; also switches media boundaries to circular.
    pub screen: Option<GroundScreen>,
}

impl Default for RealGround {
    fn default() -> Self {
        Self { media: vec![Medium::default()], screen: None }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct Impedance {
    pub r: f64,
    pub x: f64,
}

/// Angular ranges of the back lobe searched for the F/B ratio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct FbRange {
    pub horizontal: i32,
    pub vertical: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Environment {
    pub ground: GroundKind,
    /// Height added to every wire when a ground is present (metres).
    pub height: f64,
    pub wire_material: WireMaterial,
    pub swr_reference: Impedance,
    pub fb_range: FbRange,
    #[serde(default)]
    pub real_ground: RealGround,
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            ground: GroundKind::FreeSpace,
            height: 20.0,
            wire_material: WireMaterial::Lossless,
            swr_reference: Impedance { r: 50.0, x: 0.0 },
            fb_range: FbRange { horizontal: 120, vertical: 60 },
            real_ground: RealGround::default(),
        }
    }
}

impl Environment {
    /// MMANA's packed F/B range (`horizontal | vertical << 16`).
    pub fn fbr_code(&self) -> i32 {
        self.fb_range.horizontal | (self.fb_range.vertical << 16)
    }
}

/// An antenna together with the environment it is evaluated in; this is what
/// a `.maa` file stores.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct Project {
    pub antenna: Antenna,
    pub environment: Environment,
}
