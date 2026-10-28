//! Element editing (MMANA "エレメント編集").
//!
//! Wires joined at their ends form an element (a dipole, a quad loop, ...).
//! The editor recognises each element's form, lets its width, height,
//! perimeter, radius and segmentation be changed as a whole, and places the
//! elements along X by interval or position.
//!
//! Port of `CWGroup` / `CWGroupList` (ComLib.cpp) and of the edit rules of the
//! element dialog `TGrpWireDlg` (GrpWire.cpp). The arithmetic and the order of
//! comparisons follow the original statement by statement, so edited wires are
//! bit-identical to MMANA's; `tests/elements.rs` checks this against the
//! original C++ (`reference/oracle/elements_main.cpp`).
//!
//! The dialog edits its own copy of the wires and writes them back on OK.
//! [`edit_antenna`] does the same for a list of [`ElementOp`]s, replaying them
//! from the antenna each time, so callers need not keep any editor state.

use serde::{Deserialize, Serialize};

#[cfg(feature = "openapi")]
use utoipa::ToSchema;

use crate::engine::to_half_width;
use crate::model::{Antenna, Point3, SPEED_OF_LIGHT_MMANA, Segmentation, Wire};

#[allow(clippy::excessive_precision)]
const COS30: f64 = 0.866025403784438646763723170752936;
#[allow(clippy::excessive_precision)]
const TAN22R5: f64 = 0.414213562373095048801688724209698;
#[allow(clippy::excessive_precision)]
const SIN22R5: f64 = 0.382683432365089771728459984030399;
#[allow(clippy::excessive_precision)]
const COS22R5: f64 = 0.923879532511286756128183189396788;

const X: usize = 0;
const Y: usize = 1;
const Z: usize = 2;

/// What of an element the table describes (`Group::describes_wires`), and
/// its bounds.
type Described = ([bool; 4], ([f64; 3], [f64; 3]));

/// The form of an element (`CWTYPE`), as recognised from its wires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum ElementKind {
    /// 不明
    #[default]
    Unknown,
    /// 垂直DP
    VerticalDipole,
    /// 垂直□
    VerticalSquare,
    /// 垂直◇
    VerticalDiamond,
    /// 垂直△ (apex up)
    VerticalTriangle,
    /// 垂直▽ (apex down)
    VerticalInvertedTriangle,
    /// 垂直○ (octagon)
    VerticalOctagon,
    /// 垂直ループ (any other vertical loop)
    VerticalLoop,
    /// 水平DP
    HorizontalDipole,
    /// 水平□
    HorizontalSquare,
    /// 水平◇
    HorizontalDiamond,
    /// 水平△
    HorizontalTriangle,
    /// 水平▽
    HorizontalInvertedTriangle,
    /// 水平ループ (any other horizontal loop)
    HorizontalLoop,
    /// 水平系 (open, mostly horizontal)
    HorizontalOther,
    /// 垂直系 (open, mostly vertical)
    VerticalOther,
    /// 垂直双□ (two squares side by side; a template only, recognised as 垂直□)
    VerticalDoubleSquare,
    /// 垂直双▽ (bow tie; a template only, recognised as 垂直ループ)
    VerticalDoubleTriangle,
}

impl ElementKind {
    /// Whether [`ElementOp::SetKind`] can build an element of this form
    /// (the forms of the dialog's "ｴﾚﾒﾝﾄ形式の変更" menu).
    pub fn is_template(self) -> bool {
        use ElementKind::*;
        matches!(
            self,
            HorizontalDipole
                | VerticalDipole
                | VerticalSquare
                | VerticalDiamond
                | VerticalInvertedTriangle
                | VerticalTriangle
                | VerticalOctagon
                | VerticalDoubleSquare
                | VerticalDoubleTriangle
                | HorizontalSquare
                | HorizontalDiamond
                | HorizontalTriangle
                | HorizontalInvertedTriangle
        )
    }

    /// The template (`CWTYPE` of `SetNewType`) that builds this form. The
    /// horizontal triangle templates come out as the other triangle when
    /// recognised; MMANA's menu names them by the result, and so does this.
    fn template(self) -> ElementKind {
        match self {
            ElementKind::HorizontalTriangle => ElementKind::HorizontalInvertedTriangle,
            ElementKind::HorizontalInvertedTriangle => ElementKind::HorizontalTriangle,
            k => k,
        }
    }
}

/// One edit in the element editor. `row` is the 0-based element; the row
/// after the last one (`row` = element count) is the dialog's "新規" row,
/// where an edit first creates a new element.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum ElementOp {
    /// Distance to the neighbouring element on the base element's side (間隔).
    SetInterval { row: usize, value: f64 },
    /// Distance from the first element (位置).
    SetPosition { row: usize, value: f64 },
    /// Extent along Y (横幅). `scaleAll` scales every coordinate instead of
    /// moving only the outermost ones.
    #[serde(rename_all = "camelCase")]
    SetWidth {
        row: usize,
        value: f64,
        #[serde(default)]
        scale_all: bool,
    },
    /// Extent along Z (縦幅).
    #[serde(rename_all = "camelCase")]
    SetHeight {
        row: usize,
        value: f64,
        #[serde(default)]
        scale_all: bool,
    },
    /// Perimeter of a loop (全周), or the extent along X of other forms (Ｘ幅).
    #[serde(rename_all = "camelCase")]
    SetPerimeter {
        row: usize,
        value: f64,
        #[serde(default)]
        scale_all: bool,
    },
    /// Radius of every wire of the element in metres (negative selects a
    /// combined wire). Ignored when the wires' radii differ.
    SetRadius { row: usize, value: f64 },
    /// Ignored when the wires' segmentations differ.
    SetSegmentation { row: usize, segmentation: Segmentation },
    /// Rebuilds the element in another form, keeping its size and radius.
    SetKind { row: usize, kind: ElementKind },
    /// Inserts a copy of the element at `row` before it.
    Insert { row: usize },
    /// Deletes an element; the base element cannot be deleted.
    Delete { row: usize },
    /// Rounds sizes, positions and radii to `decimals` places (値を丸める),
    /// in metres or, with `inMillimetres`, in millimetres (通常単位をmmで表示),
    /// or, with `inWavelengths`, in wavelengths (which takes precedence). The
    /// radius is rounded in millimetres unless `inWavelengths` is set.
    #[serde(rename_all = "camelCase")]
    RoundValues {
        decimals: u32,
        #[serde(default)]
        in_wavelengths: bool,
        #[serde(default)]
        in_millimetres: bool,
        #[serde(default)]
        scale_all: bool,
    },
}

/// One line of the element table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ElementRow {
    pub kind: ElementKind,
    /// Distance to the neighbouring element on the base element's side
    /// (0 for the base element).
    pub interval: f64,
    /// Distance from the first element.
    pub position: f64,
    /// Extent along Y (横幅).
    pub width: f64,
    /// Extent along Z (縦幅).
    pub height: f64,
    /// Perimeter (全周), or the extent along X when `xWidth` is set.
    pub perimeter: f64,
    /// `perimeter` is the extent along X (Ｘ幅), not a loop's perimeter.
    pub x_width: bool,
    /// Radius of the wires, or `None` when they differ (複合).
    pub radius: Option<f64>,
    /// Segmentation of the wires, or `None` when they differ (複合).
    pub segmentation: Option<Segmentation>,
    pub wire_count: usize,
}

/// The wires the editor writes back, in MMANA's order.
#[derive(Debug, Clone, PartialEq)]
pub struct EditedWires {
    pub wires: Vec<Wire>,
    /// Element (row) of each wire.
    pub elements: Vec<usize>,
    /// Slot of each wire in the editor's wire table. A slot freed by a
    /// deletion may be reused by a wire the editor creates.
    pub slots: Vec<usize>,
    /// The original wire (index into the wires the editor opened with) each
    /// wire still is, or `None` for wires the editor created.
    pub origins: Vec<Option<usize>>,
}

/// `RoundUp(d, s)`: rounds half away from zero to a multiple of 1/s.
pub(crate) fn round_up(d: f64, s: f64) -> f64 {
    if d >= 0.0 {
        let mut d = d * s;
        d += 0.5;
        d = d.trunc();
        d / s
    } else {
        let mut d = -d;
        d *= s;
        d += 0.5;
        d = d.trunc();
        -(d / s)
    }
}

fn coord(p: &mut Point3, axis: usize) -> &mut f64 {
    match axis {
        X => &mut p.x,
        Y => &mut p.y,
        _ => &mut p.z,
    }
}

/// A wire of the editor's own wire table (`CWGroupList::wdef`). Deleted
/// wires stay as dead slots (`PNo == 0`) until a new wire reuses them.
#[derive(Debug, Clone)]
struct Slot {
    wire: Wire,
    live: bool,
    /// The original wire this slot holds. Not part of the original: it lets
    /// sources and loads follow their wire, and is cleared when the wire is
    /// deleted so that a new wire reusing the slot does not inherit them.
    origin: Option<usize>,
}

/// `WREF`: a wire of an element and the values `AnaType` derives from it.
#[derive(Debug, Clone, Copy, Default)]
struct WireRef {
    slot: usize,
    zz: f64,
    vxy: f64,
    vyz: f64,
    /// 1-based place in the loop, 0 = not visited.
    lcf: i32,
}

/// A corner of a loop (`LOOPS`).
#[derive(Debug, Clone, Copy)]
struct Vertex {
    p: Point3,
    vxy: f64,
    vyz: f64,
}

/// `CWGroup`: one element.
#[derive(Debug, Clone)]
struct Group {
    refs: Vec<WireRef>,
    /// Bounds (`XL` `YL` `ZL` / `XH` `YH` `ZH`); X is relative to `c[X]`.
    lo: [f64; 3],
    hi: [f64; 3],
    /// Centre (`BX` `BY` `BZ`); `c[X]` is where the element was found.
    c: [f64; 3],
    /// `XwF`: not a loop of known form, so the perimeter column is the X extent.
    xw: bool,
    kind: ElementKind,
    enb_r: bool,
    enb_seg: bool,
}

impl Group {
    /// `CWGroup::Init`
    fn new() -> Self {
        Self {
            refs: Vec::new(),
            lo: [0.0; 3],
            hi: [0.0; 3],
            c: [0.0; 3],
            xw: false,
            kind: ElementKind::Unknown,
            enb_r: true,
            enb_seg: true,
        }
    }

    fn add(&mut self, slot: usize) {
        self.refs.push(WireRef { slot, ..WireRef::default() });
    }

    /// `AnaLoop`: follows the wires end to end from `p`; true when the chain
    /// closes at `o`. Like the original it gives up at the first dead end.
    fn ana_loop(&mut self, slots: &[Slot], n: i32, o: Point3, p: Point3) -> bool {
        for i in 0..self.refs.len() {
            if self.refs[i].lcf != 0 {
                continue;
            }
            let w = &slots[self.refs[i].slot].wire;
            let next = if p == w.start {
                w.end
            } else if p == w.end {
                w.start
            } else {
                continue;
            };
            self.refs[i].lcf = n + 1;
            if o == next || self.ana_loop(slots, n + 1, o, next) {
                return true;
            }
            // (the original marks the wire a dead end here, -1; a chain that
            // fails is never read again, as the search gives up)
            return false;
        }
        // (here the original also marks the slot after the last wire as a dead end)
        false
    }

    /// `AnaType`: recognises the form.
    fn ana_type(&mut self, slots: &[Slot]) {
        for r in &mut self.refs {
            let w = &slots[r.slot].wire;
            r.lcf = 0;
            let xx = w.end.x - w.start.x;
            let yy = w.end.y - w.start.y;
            r.zz = w.end.z - w.start.z;
            r.vxy = if yy != 0.0 { if xx != 0.0 { round_up(yy / xx, 10.0) } else { 1.0 } } else { 0.0 };
            r.vyz = if r.zz != 0.0 { if yy != 0.0 { round_up(r.zz / yy, 10.0) } else { 1.0 } } else { 0.0 };
        }
        // The original means to start from the longest wire, but its maximum
        // starts at 0 and is never raised, so it always starts from the first.
        let first = slots[self.refs[0].slot].wire.clone();
        self.refs[0].lcf = 1;
        self.xw = false;
        if !self.ana_loop(slots, 1, first.start, first.end) {
            self.xw = true;
            self.kind = if self.hi[Z] != self.lo[Z] {
                if self.lo[X] == self.hi[X] && self.lo[Y] == self.hi[Y] {
                    ElementKind::VerticalDipole
                } else if (self.hi[Z] - self.lo[Z]) >= (self.width() / 4.0) {
                    ElementKind::VerticalOther
                } else {
                    ElementKind::HorizontalOther
                }
            } else if self.lo[X] == self.hi[X] {
                ElementKind::HorizontalDipole
            } else {
                ElementKind::HorizontalOther
            };
            return;
        }

        // the loop's wires in order of travel
        let mut order = vec![0usize; self.refs.len()];
        let mut nm = 0;
        for (i, o) in order.iter_mut().enumerate() {
            if let Some(j) = self.refs.iter().position(|r| i as i32 == r.lcf - 1) {
                *o = j;
                nm += 1;
            }
        }

        // corners: where the direction changes
        let mut lp: Vec<Vertex> = Vec::with_capacity(nm);
        let mut zz = 0.0;
        let wire_bounds = (self.lo, self.hi);
        self.lo = [f64::MAX; 3];
        self.hi = [-f64::MAX; 3];
        for i in 0..nm {
            let j = if i + 1 >= nm { 0 } else { i + 1 };
            let rp = self.refs[order[i]];
            zz += rp.zz.abs();
            let rrp = self.refs[order[j]];
            if rp.vxy == rrp.vxy && rp.vyz == rrp.vyz {
                continue;
            }
            let (a, b) = (&slots[rp.slot].wire, &slots[rrp.slot].wire);
            let p = if a.start == b.start || a.start == b.end { a.start } else { a.end };
            let v = [p.x, p.y, p.z];
            for axis in [X, Y, Z] {
                if self.lo[axis] > v[axis] {
                    self.lo[axis] = v[axis];
                }
            }
            for axis in [X, Y, Z] {
                if self.hi[axis] < v[axis] {
                    self.hi[axis] = v[axis];
                }
            }
            lp.push(Vertex { p, vxy: rp.vxy, vyz: rp.vyz });
        }
        if lp.is_empty() {
            // a closed chain that never turns (a wire and its reverse): the
            // original keeps the empty bounds, ±f64::MAX, and shows sizes of -inf
            (self.lo, self.hi) = wire_bounds;
        }

        use ElementKind::*;
        let nt = lp.len();
        if nt == 3 {
            // △ / ▽
            let (a, b, c) =
                if zz != 0.0 { (lp[0].p.z, lp[1].p.z, lp[2].p.z) } else { (lp[0].p.x, lp[1].p.x, lp[2].p.x) };
            let apex_up = if a == b {
                c > a
            } else if a == c {
                b > a
            } else {
                a > b
            };
            self.kind = match (zz != 0.0, apex_up) {
                (true, true) => VerticalTriangle,
                (true, false) => VerticalInvertedTriangle,
                (false, true) => HorizontalTriangle,
                (false, false) => HorizontalInvertedTriangle,
            };
        } else if nt == 4 {
            // □ / ◇
            self.kind = if zz != 0.0 {
                if lp[0].vxy != 0.0 && lp[0].vyz != 0.0 { VerticalDiamond } else { VerticalSquare }
            } else if lp[0].vxy == 0.0 || lp[0].vxy == 1.0 {
                HorizontalSquare
            } else {
                HorizontalDiamond
            };
        } else if nt >= 3 {
            // other loops
            self.xw = true;
            self.kind = if zz != 0.0 {
                if nt == 8 {
                    self.xw = false;
                    VerticalOctagon
                } else {
                    VerticalLoop
                }
            } else {
                HorizontalLoop
            };
        } else {
            self.xw = true;
            self.kind = Unknown;
        }
    }

    /// `AnaWire`: bounds, common radius / segmentation, form; then moves the
    /// element's X to be relative to its centre.
    fn ana_wire(&mut self, slots: &mut [Slot]) {
        self.enb_r = true;
        self.enb_seg = true;
        if self.refs.is_empty() {
            return;
        }
        self.lo = [f64::MAX; 3];
        self.hi = [-f64::MAX; 3];
        let (mut r0, mut seg0) = (0.0, 0);
        for (i, r) in self.refs.iter().enumerate() {
            let w = &slots[r.slot].wire;
            for p in [[w.start.x, w.start.y, w.start.z], [w.end.x, w.end.y, w.end.z]] {
                for axis in [X, Y, Z] {
                    if self.lo[axis] > p[axis] {
                        self.lo[axis] = p[axis];
                    }
                }
                for axis in [X, Y, Z] {
                    if self.hi[axis] < p[axis] {
                        self.hi[axis] = p[axis];
                    }
                }
            }
            let seg = w.segmentation.to_code();
            if i > 0 {
                if w.radius != r0 {
                    self.enb_r = false;
                }
                if seg != seg0 {
                    self.enb_seg = false;
                }
            } else {
                r0 = w.radius;
                seg0 = seg;
            }
        }
        self.ana_type(slots);
        for axis in [X, Y, Z] {
            self.c[axis] = (self.hi[axis] + self.lo[axis]) / 2.0;
        }
        for r in &self.refs {
            let w = &mut slots[r.slot].wire;
            w.start.x -= self.c[X];
            w.end.x -= self.c[X];
        }
        self.lo[X] -= self.c[X];
        self.hi[X] -= self.c[X];
    }

    /// `GetHeight`
    fn height(&self) -> f64 {
        if self.hi[Z] != self.lo[Z] {
            self.hi[Z] - self.lo[Z]
        } else if !self.xw {
            self.hi[X] - self.lo[X]
        } else {
            0.0
        }
    }

    /// `GetWidth`
    fn width(&self) -> f64 {
        if self.hi[Y] != self.lo[Y] {
            self.hi[Y] - self.lo[Y]
        } else if !self.xw {
            self.hi[X] - self.lo[X]
        } else {
            0.0
        }
    }

    /// `GetRound`
    fn perimeter(&self) -> f64 {
        use ElementKind::*;
        let h = self.height();
        let w = self.width();
        match self.kind {
            VerticalSquare | HorizontalSquare => (h * 2.0) + (w * 2.0),
            VerticalDiamond | HorizontalDiamond => 2.0 * ((h * h) + (w * w)).sqrt(),
            VerticalTriangle | VerticalInvertedTriangle | HorizontalTriangle | HorizontalInvertedTriangle => {
                w + 2.0 * ((h * h) + ((w * w) / 4.0)).sqrt()
            }
            VerticalOctagon => ((h * TAN22R5) * 4.0) + ((w * TAN22R5) * 4.0),
            _ => self.hi[X] - self.lo[X],
        }
    }

    /// `GetR`
    fn radius(&self, slots: &[Slot]) -> f64 {
        self.refs.first().map_or(0.005, |r| slots[r.slot].wire.radius)
    }

    /// `GetSEG`
    fn segmentation(&self, slots: &[Slot]) -> Segmentation {
        self.refs.first().map_or(Segmentation::Auto, |r| slots[r.slot].wire.segmentation)
    }

    /// New bounds for an extent of `w` along Y or Z (`SetWidth`, `SetHeight`).
    fn new_bounds(&self, axis: usize, w: f64) -> (f64, f64) {
        let (lo, hi) = (self.lo[axis], self.hi[axis]);
        if lo == 0.0 {
            (0.0, w)
        } else if hi == 0.0 {
            (-w, 0.0)
        } else if lo < 0.0 && hi > 0.0 {
            (lo * (w / (hi - lo)), hi * (w / (hi - lo)))
        } else {
            (self.c[axis] - (w / 2.0), self.c[axis] + (w / 2.0))
        }
    }

    /// Moves the coordinates at (or beyond) the bounds to new bounds
    /// (the edge-only `SetXWidth` / `SetWidth` / `SetHeight`). Each point
    /// moves once, by where it was: the original tests each case after the
    /// one before, so a point beyond a bound that the move lands exactly on
    /// the old bound moves again (a 5 cm stub below a loop that shrinks by
    /// 5 cm vanishes).
    fn move_ends(&mut self, slots: &mut [Slot], axis: usize, nl: f64, nh: f64) {
        let (lo, hi) = (self.lo[axis], self.hi[axis]);
        for r in &self.refs {
            let w = &mut slots[r.slot].wire;
            for p in [&mut w.start, &mut w.end] {
                let t = coord(p, axis);
                if *t > hi {
                    *t += nh - hi;
                } else if *t < lo {
                    *t -= lo - nl;
                } else if *t == lo {
                    *t = nl;
                } else if *t == hi {
                    *t = nh;
                }
            }
        }
        self.lo[axis] = nl;
        self.hi[axis] = nh;
    }

    /// Scales every coordinate to the new extent (`SetXWidthAll` /
    /// `SetWidthAll` / `SetHeightAll` with `SetLineXY`). Like the original
    /// this scales about 0, not about the element centre.
    fn scale_coords(&mut self, slots: &mut [Slot], axis: usize, nl: f64, nh: f64) {
        let (ol, oh) = (self.lo[axis], self.hi[axis]);
        for r in &self.refs {
            let w = &mut slots[r.slot].wire;
            for p in [&mut w.start, &mut w.end] {
                let ow = oh - ol;
                let nw = nh - nl;
                let t = coord(p, axis);
                *t *= nw / ow;
            }
        }
        self.lo[axis] = nl;
        self.hi[axis] = nh;
    }

    /// `SetXWidth` / `SetXWidthAll`
    fn set_x_width(&mut self, slots: &mut [Slot], all: bool, w: f64) {
        if self.hi[X] != self.lo[X] {
            let (nl, nh) = (-(w / 2.0), w / 2.0);
            if all {
                self.scale_coords(slots, X, nl, nh);
            } else {
                self.move_ends(slots, X, nl, nh);
            }
        }
    }

    /// `SetWidth` / `SetHeight` and their `All` versions: an extent along Y or
    /// Z, or along X when the element is flat in that direction.
    fn set_extent(&mut self, slots: &mut [Slot], axis: usize, all: bool, w: f64) {
        if self.hi[axis] != self.lo[axis] {
            let (nl, nh) = self.new_bounds(axis, w);
            if all {
                self.scale_coords(slots, axis, nl, nh);
            } else {
                self.move_ends(slots, axis, nl, nh);
            }
        } else {
            self.set_x_width(slots, all, w);
        }
    }

    fn set_width(&mut self, slots: &mut [Slot], all: bool, w: f64) {
        self.set_extent(slots, Y, all, w);
    }

    fn set_height(&mut self, slots: &mut [Slot], all: bool, w: f64) {
        self.set_extent(slots, Z, all, w);
    }

    /// `SetRound` / `SetRoundAll`: keeps the aspect ratio of the loop.
    fn set_perimeter(&mut self, slots: &mut [Slot], all: bool, d: f64) {
        use ElementKind::*;
        let h = self.height();
        let w = self.width();
        match self.kind {
            VerticalSquare | HorizontalSquare => {
                let w = d / ((h / w) + 1.0);
                self.set_width(slots, all, w / 2.0);
                self.set_height(slots, all, (d - w) / 2.0);
            }
            VerticalDiamond | HorizontalDiamond => {
                let b = h / w;
                let w = d / (4.0 * ((b * b) + 1.0)).sqrt();
                let h = b * w;
                self.set_width(slots, all, w);
                self.set_height(slots, all, h);
            }
            VerticalTriangle | VerticalInvertedTriangle | HorizontalTriangle | HorizontalInvertedTriangle => {
                let b = h / w;
                let w = d / (2.0 * (b * b + 0.25).sqrt() + 1.0);
                let h = b * w;
                self.set_width(slots, all, w);
                self.set_height(slots, all, h);
            }
            VerticalOctagon => self.set_octagon_perimeter(slots, d),
            _ => self.set_x_width(slots, all, d),
        }
    }

    /// `SetPieRound`: a regular octagon of perimeter `d`.
    fn set_octagon_perimeter(&mut self, slots: &mut [Slot], d: f64) {
        let l = d / 8.0;
        let h = l / TAN22R5;
        self.set_width(slots, false, h);
        self.set_height(slots, false, h);
        let rsin = l / 2.0;
        let (by, bz) = (self.c[Y], self.c[Z]);
        let (yl, yh, zl, zh) = (self.lo[Y], self.hi[Y], self.lo[Z], self.hi[Z]);
        for r in &self.refs {
            let w = &mut slots[r.slot].wire;
            if w.start.z == zh || w.start.z == zl {
                w.start.y = if w.start.y > by { by + rsin } else { by - rsin };
            }
            if w.end.z == zh || w.end.z == zl {
                w.end.y = if w.end.y > by { by + rsin } else { by - rsin };
            }
            if w.start.y == yh || w.start.y == yl {
                w.start.z = if w.start.z > bz { bz + rsin } else { bz - rsin };
            }
            if w.end.y == yh || w.end.y == yl {
                w.end.z = if w.end.z > bz { bz + rsin } else { bz - rsin };
            }
        }
    }

    /// `SetR`
    fn set_radius(&mut self, slots: &mut [Slot], r: f64) {
        if self.enb_r {
            for wr in &self.refs {
                slots[wr.slot].wire.radius = r;
            }
        }
    }

    /// `SetSeg`
    fn set_segmentation(&mut self, slots: &mut [Slot], s: Segmentation) {
        if self.enb_seg {
            for wr in &self.refs {
                slots[wr.slot].wire.segmentation = s;
            }
        }
    }

    /// Whether the bounds the table shows are the extent of the wires (to
    /// 10 ppm, at least 0.01 mm): the size along X, Y and Z, and the centre
    /// along X.
    fn describes_wires(&self, slots: &[Slot]) -> [bool; 4] {
        let ((lo, hi), near) = self.extent(slots);
        let size = |axis: usize| near(self.hi[axis] - self.lo[axis], hi[axis] - lo[axis]);
        [size(X), size(Y), size(Z), near(self.lo[X] / 2.0 + self.hi[X] / 2.0, lo[X] / 2.0 + hi[X] / 2.0)]
    }

    /// Moves bounds along Y and Z that have the size of the wires but not
    /// their place back onto them. Scaling every coordinate scales about 0,
    /// not about the element's centre, so an element off 0 moves while its
    /// bounds stay centred where it was; the next edit that moves only the
    /// outermost points would then look for them in the wrong place.
    fn follow_wires(&mut self, slots: &[Slot]) {
        let ((lo, hi), near) = self.extent(slots);
        for axis in [Y, Z] {
            if near(self.hi[axis] - self.lo[axis], hi[axis] - lo[axis]) && !near(self.lo[axis], lo[axis]) {
                (self.lo[axis], self.hi[axis]) = (lo[axis], hi[axis]);
                self.c[axis] = (lo[axis] + hi[axis]) / 2.0;
            }
        }
    }

    /// The extent of the wires, and a comparison to 10 ppm of their size
    /// (at least 0.01 mm).
    #[allow(clippy::type_complexity)]
    fn extent(&self, slots: &[Slot]) -> (([f64; 3], [f64; 3]), impl Fn(f64, f64) -> bool + use<>) {
        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        for r in &self.refs {
            let w = &slots[r.slot].wire;
            for p in [[w.start.x, w.start.y, w.start.z], [w.end.x, w.end.y, w.end.z]] {
                for axis in [X, Y, Z] {
                    lo[axis] = lo[axis].min(p[axis]);
                    hi[axis] = hi[axis].max(p[axis]);
                }
            }
        }
        // 10 ppm of the element's size, at least 0.01 mm: far below what can
        // be built, and above the micrometres the original leaves behind when
        // an earlier edit moved an end by an ulp, so that it no longer finds it
        let tol = (1e-5 * [X, Y, Z].iter().fold(0.0, |m: f64, &axis| m.max(hi[axis] - lo[axis]))).max(1e-5);
        ((lo, hi), move |a: f64, b: f64| (a - b).abs() <= tol)
    }

    fn same_shape(&self, other: &Group, slots: &[Slot]) -> bool {
        self.refs.len() == other.refs.len()
            && self.refs.iter().zip(&other.refs).all(|(a, b)| {
                let (a, b) = (&slots[a.slot].wire, &slots[b.slot].wire);
                a.start == b.start && a.end == b.end
            })
    }
}

/// The element table of an antenna (`CWGroupList`): its elements sorted along
/// X and the spacing between them.
#[derive(Debug, Clone)]
pub struct ElementList {
    slots: Vec<Slot>,
    groups: Vec<Group>,
    /// `BaseW`: the element nearest to X = 0, from which intervals count.
    base: usize,
    /// `Intval`: for elements before the base, the distance to the next
    /// element; after it, the distance to the previous one. Like the
    /// original's fixed array it keeps stale values past the last element.
    intervals: Vec<f64>,
    /// `exeenv.rmd`
    wavelength: f64,
}

impl ElementList {
    /// Groups the wires into elements (`SetWire`). `wavelength` is the design
    /// wavelength (λ = 299.8 / f), used for the sizes of new elements.
    pub fn new(wires: &[Wire], wavelength: f64) -> Self {
        let slots: Vec<Slot> =
            wires.iter().enumerate().map(|(i, w)| Slot { wire: w.clone(), live: true, origin: Some(i) }).collect();

        // wires joined end to end (`Connect`)
        let n = slots.len();
        let mut member: Vec<Option<usize>> = vec![None; n];
        let mut count = 0;
        for i in 0..n {
            if member[i].is_some() {
                continue;
            }
            member[i] = Some(count);
            let mut open = vec![slots[i].wire.start, slots[i].wire.end];
            while let Some(p) = open.pop() {
                for k in 0..n {
                    if member[k].is_some() {
                        continue;
                    }
                    let w = &slots[k].wire;
                    if w.start == p {
                        member[k] = Some(count);
                        open.push(w.end);
                    } else if w.end == p {
                        member[k] = Some(count);
                        open.push(w.start);
                    }
                }
            }
            count += 1;
        }
        let mut groups: Vec<Group> = (0..count).map(|_| Group::new()).collect();
        for (k, g) in member.iter().enumerate() {
            groups[g.expect("every wire belongs to an element")].add(k);
        }

        let mut list = Self { slots, groups: Vec::new(), base: 0, intervals: Vec::new(), wavelength };
        for g in &mut groups {
            g.ana_wire(&mut list.slots);
        }
        list.groups = groups;
        list.sort();
        list
    }

    /// `Sort`: orders the elements by X (the original's shell sort, so equal
    /// positions keep its order) and derives the intervals.
    fn sort(&mut self) {
        let n = self.groups.len();
        let mut gap = n / 2;
        while gap > 0 {
            for i in gap..n {
                let mut j = i as isize - gap as isize;
                while j >= 0 {
                    let ju = j as usize;
                    if self.groups[ju].c[X] <= self.groups[ju + gap].c[X] {
                        break;
                    }
                    self.groups.swap(ju, ju + gap);
                    j -= gap as isize;
                }
            }
            gap /= 2;
        }
        // the base element is the one nearest to X = 0
        let mut minx = f64::MAX;
        self.base = 0;
        for (i, g) in self.groups.iter().enumerate() {
            if minx > g.c[X].abs() {
                minx = g.c[X].abs();
                self.base = i;
            }
        }
        self.intervals = vec![0.0; n.max(1)];
        if n == 0 {
            return;
        }
        let mut d = self.groups[self.base].c[X];
        for i in (0..=self.base).rev() {
            self.intervals[i] = d - self.groups[i].c[X];
            d = self.groups[i].c[X];
        }
        let mut d = self.groups[self.base].c[X];
        for i in self.base + 1..n {
            self.intervals[i] = self.groups[i].c[X] - d;
            d = self.groups[i].c[X];
        }
    }

    pub fn len(&self) -> usize {
        self.groups.len()
    }

    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }

    /// The element intervals count from (`BaseW`).
    pub fn base(&self) -> usize {
        self.base
    }

    fn interval(&self, i: usize) -> f64 {
        self.intervals.get(i).copied().unwrap_or(0.0)
    }

    fn interval_mut(&mut self, i: usize) -> &mut f64 {
        if self.intervals.len() <= i {
            self.intervals.resize(i + 1, 0.0);
        }
        &mut self.intervals[i]
    }

    /// `GetXPosition`: distance of element `n` from the first element.
    fn position(&self, n: usize) -> f64 {
        let mut d = 0.0;
        let (mut i, mut n) = (0, n);
        while i < n && i < self.groups.len() {
            if i == self.base {
                i += 1;
                n += 1;
            }
            d += self.interval(i);
            i += 1;
        }
        d
    }

    /// `SetXPosition`: moves element `n` without moving the others.
    fn set_position(&mut self, n: usize, d: f64) {
        let d = d - self.position(n);
        let len = self.groups.len();
        if n == self.base {
            if n > 0 {
                *self.interval_mut(n - 1) += d;
            }
            if n + 1 < len {
                *self.interval_mut(n + 1) -= d;
            }
        } else if n < self.base {
            if n > 0 {
                *self.interval_mut(n - 1) += d;
            }
            *self.interval_mut(n) -= d;
        } else {
            *self.interval_mut(n) += d;
            if n + 1 < len {
                *self.interval_mut(n + 1) -= d;
            }
        }
    }

    pub fn rows(&self) -> Vec<ElementRow> {
        self.groups
            .iter()
            .enumerate()
            .map(|(i, g)| ElementRow {
                kind: g.kind,
                interval: self.interval(i),
                position: self.position(i),
                width: g.width(),
                height: g.height(),
                perimeter: g.perimeter(),
                x_width: g.xw,
                radius: g.enb_r.then(|| g.radius(&self.slots)),
                segmentation: g.enb_seg.then(|| g.segmentation(&self.slots)),
                wire_count: g.refs.len(),
            })
            .collect()
    }

    /// `GetNewWire`: the first free slot, or a new one.
    fn new_slot(&mut self) -> usize {
        if let Some(i) = self.slots.iter().position(|s| !s.live) {
            return i;
        }
        let blank = Wire {
            start: Point3::default(),
            end: Point3::default(),
            radius: 0.0,
            segmentation: Segmentation::Auto,
            note: String::new(),
            group: String::new(),
        };
        self.slots.push(Slot { wire: blank, live: false, origin: None });
        self.slots.len() - 1
    }

    /// `AddWire`: a template wire. Like the original, a reused slot keeps its note.
    #[allow(clippy::too_many_arguments)]
    fn add_wire(&mut self, x1: f64, y1: f64, z1: f64, x2: f64, y2: f64, z2: f64, r: f64, seg: i32) -> usize {
        let i = self.new_slot();
        let s = &mut self.slots[i];
        s.wire.start = Point3::new(x1, y1, z1);
        s.wire.end = Point3::new(x2, y2, z2);
        s.wire.radius = r;
        s.wire.segmentation = Segmentation::from_code(seg);
        s.live = true;
        s.origin = None;
        i
    }

    /// A new element with copies of the wires of element `src`.
    fn copy_of(&mut self, src: usize) -> Group {
        let mut g = Group::new();
        let from: Vec<usize> = self.groups[src].refs.iter().map(|r| r.slot).collect();
        for s in from {
            let i = self.new_slot();
            self.slots[i] = Slot { origin: None, ..self.slots[s].clone() };
            g.add(i);
        }
        g.ana_wire(&mut self.slots);
        g
    }

    /// `CreateWire`: a new last element, copied from the one before it.
    fn create(&mut self, kind: ElementKind) {
        let w = self.groups.len();
        if w == 0 {
            self.set_kind(w, kind);
            return;
        }
        let g = self.copy_of(w - 1);
        self.groups.push(g);
        let d = if self.interval(w - 1) != 0.0 {
            self.interval(w - 1)
        } else if w >= 2 && self.interval(w - 2) != 0.0 {
            self.interval(w - 2)
        } else {
            self.wavelength / 6.0
        };
        *self.interval_mut(w) = d;
    }

    /// `InsertItem`
    fn insert(&mut self, w: usize) {
        let len = self.groups.len();
        if w >= len {
            self.create(ElementKind::HorizontalDipole);
            return;
        }
        let g = self.copy_of(w);
        self.groups.insert(w, g);
        self.interval_mut(len);
        for i in (w + 1..=len).rev() {
            self.intervals[i] = self.intervals[i - 1];
        }
        let d = if self.interval(w + 1) != 0.0 {
            self.interval(w + 1)
        } else if w > 0 && self.interval(w - 1) != 0.0 {
            self.interval(w - 1)
        } else {
            self.wavelength / 6.0
        };
        *self.interval_mut(w) = d;
        if w <= self.base {
            self.base += 1;
        }
    }

    /// `DeleteWire`: frees the element's wire slots.
    fn free_wires(&mut self, n: usize) {
        for r in &self.groups[n].refs {
            self.slots[r.slot].live = false;
            self.slots[r.slot].origin = None;
        }
    }

    /// `DeleteItem` (never called for the base element).
    fn delete(&mut self, n: usize) {
        let len = self.groups.len();
        self.free_wires(n);
        self.groups.remove(n);
        for i in n..len - 1 {
            // (an element made by `set_kind` in the new row has no interval yet)
            let d = self.interval(i + 1);
            *self.interval_mut(i) = d;
        }
        if n <= self.base {
            self.base -= 1;
        }
    }

    /// `SetNewType`: rebuilds element `n` (or creates it when `n` is the
    /// element count) from the template `kind` (see [`ElementKind::template`]),
    /// keeping its size, radius and height. `n` is at most the element count
    /// and `kind` a template (the callers check).
    fn set_kind(&mut self, n: usize, kind: ElementKind) {
        use ElementKind::*;
        let len = self.groups.len();
        let rmd = if self.wavelength != 0.0 { self.wavelength } else { 23.0 };
        let (mut z, mut round, mut width, mut height, mut rad) = (0.0, rmd, rmd / 2.0, rmd / 2.0, 0.005);
        // the original wires the old element's wires were, in order
        let mut freed: Vec<Option<usize>> = Vec::new();
        if n < len {
            let g = &self.groups[n];
            if kind == g.kind {
                return;
            }
            z = g.c[Z];
            round = g.perimeter();
            width = g.width();
            height = g.height();
            rad = g.radius(&self.slots);
            freed = g.refs.iter().map(|r| self.slots[r.slot].origin).collect();
            self.free_wires(n);
        }
        // size of the new element: the old perimeter, else twice the width or
        // height, else λ (scaled for the forms below)
        let size = |k: f64, k2: f64, fallback: f64| {
            if round != 0.0 {
                round * k
            } else if width != 0.0 {
                width * k2
            } else if height != 0.0 {
                height * k2
            } else {
                fallback
            }
        };
        let w = |list: &mut Self, x1, y1, z1, x2, y2, z2, seg| list.add_wire(x1, y1, z1, x2, y2, z2, rad, seg);
        let mut wires = Vec::new();
        match kind {
            HorizontalDipole => wires.push(w(self, 0.0, -1.0, 0.0, 0.0, 1.0, 0.0, 0)),
            VerticalDipole => wires.push(w(self, 0.0, 0.0, -1.0, 0.0, 0.0, 1.0, 0)),
            VerticalSquare => {
                wires.push(w(self, 0.0, -1.0, -1.0, 0.0, 1.0, -1.0, -1));
                wires.push(w(self, 0.0, 1.0, -1.0, 0.0, 1.0, 1.0, -1));
                wires.push(w(self, 0.0, 1.0, 1.0, 0.0, -1.0, 1.0, -1));
                wires.push(w(self, 0.0, -1.0, 1.0, 0.0, -1.0, -1.0, -1));
            }
            VerticalDiamond => {
                wires.push(w(self, 0.0, 1.0, 0.0, 0.0, 0.0, -1.0, -1));
                wires.push(w(self, 0.0, 0.0, -1.0, 0.0, -1.0, 0.0, -1));
                wires.push(w(self, 0.0, -1.0, 0.0, 0.0, 0.0, 1.0, -1));
                wires.push(w(self, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, -1));
            }
            VerticalTriangle => {
                wires.push(w(self, 0.0, COS30, -0.5, 0.0, -COS30, -0.5, -1));
                wires.push(w(self, 0.0, -COS30, -0.5, 0.0, 0.0, 1.0, -1));
                wires.push(w(self, 0.0, 0.0, 1.0, 0.0, COS30, -0.5, -1));
            }
            VerticalInvertedTriangle => {
                wires.push(w(self, 0.0, COS30, 0.5, 0.0, -COS30, 0.5, -1));
                wires.push(w(self, 0.0, -COS30, 0.5, 0.0, 0.0, -1.0, -1));
                wires.push(w(self, 0.0, 0.0, -1.0, 0.0, COS30, 0.5, -1));
            }
            VerticalOctagon => {
                let (s, c) = (SIN22R5, COS22R5);
                wires.push(w(self, 0.0, s, -c, 0.0, -s, -c, -1));
                wires.push(w(self, 0.0, -s, -c, 0.0, -c, -s, -1));
                wires.push(w(self, 0.0, -c, -s, 0.0, -c, s, -1));
                wires.push(w(self, 0.0, -c, s, 0.0, -s, c, -1));
                wires.push(w(self, 0.0, -s, c, 0.0, s, c, -1));
                wires.push(w(self, 0.0, s, c, 0.0, c, s, -1));
                wires.push(w(self, 0.0, c, s, 0.0, c, -s, -1));
                wires.push(w(self, 0.0, c, -s, 0.0, s, -c, -1));
            }
            VerticalDoubleSquare => {
                wires.push(w(self, 0.0, -2.0, 1.0, 0.0, 0.0, 1.0, -1));
                wires.push(w(self, 0.0, 0.0, 1.0, 0.0, 2.0, 1.0, -1));
                wires.push(w(self, 0.0, 2.0, 1.0, 0.0, 2.0, -1.0, -1));
                wires.push(w(self, 0.0, 2.0, -1.0, 0.0, 0.0, -1.0, -1));
                wires.push(w(self, 0.0, 0.0, -1.0, 0.0, -2.0, -1.0, -1));
                wires.push(w(self, 0.0, -2.0, -1.0, 0.0, -2.0, 1.0, -1));
                wires.push(w(self, 0.0, 0.0, 1.0, 0.0, 0.0, -1.0, -1));
            }
            VerticalDoubleTriangle => {
                wires.push(w(self, 0.0, -2.0, 1.0, 0.0, 0.0, 0.01, -1));
                wires.push(w(self, 0.0, 0.0, 0.01, 0.0, 2.0, 1.0, -1));
                wires.push(w(self, 0.0, 2.0, 1.0, 0.0, 2.0, -1.0, -1));
                wires.push(w(self, 0.0, 2.0, -1.0, 0.0, 0.0, -0.01, -1));
                wires.push(w(self, 0.0, 0.0, -0.01, 0.0, -2.0, -1.0, -1));
                wires.push(w(self, 0.0, -2.0, -1.0, 0.0, -2.0, 1.0, -1));
                wires.push(w(self, 0.0, 0.0, 0.01, 0.0, 0.0, -0.01, -1));
            }
            HorizontalSquare => {
                wires.push(w(self, -1.0, -1.0, z, 1.0, -1.0, z, -1));
                wires.push(w(self, 1.0, -1.0, z, 1.0, 1.0, z, -1));
                wires.push(w(self, 1.0, 1.0, z, -1.0, 1.0, z, -1));
                wires.push(w(self, -1.0, 1.0, z, -1.0, -1.0, z, -1));
            }
            HorizontalDiamond => {
                wires.push(w(self, 0.0, -1.0, z, -1.0, 0.0, z, -1));
                wires.push(w(self, -1.0, 0.0, z, 0.0, 1.0, z, -1));
                wires.push(w(self, 0.0, 1.0, z, 1.0, 0.0, z, -1));
                wires.push(w(self, 1.0, 0.0, z, 0.0, -1.0, z, -1));
            }
            HorizontalTriangle => {
                wires.push(w(self, 0.5, COS30, z, 0.5, -COS30, z, -1));
                wires.push(w(self, 0.5, -COS30, z, -1.0, 0.0, z, -1));
                wires.push(w(self, -1.0, 0.0, z, 0.5, COS30, z, -1));
            }
            HorizontalInvertedTriangle => {
                wires.push(w(self, -0.5, COS30, z, -0.5, -COS30, z, -1));
                wires.push(w(self, -0.5, -COS30, z, 1.0, 0.0, z, -1));
                wires.push(w(self, 1.0, 0.0, z, -0.5, COS30, z, -1));
            }
            _ => unreachable!("not a template"),
        }
        let mut g = Group::new();
        for (k, s) in wires.into_iter().enumerate() {
            // the rebuilt element is the same element: its k-th wire takes over
            // the sources and loads of the old k-th wire, wherever its slot is
            if let Some(&origin) = freed.get(k) {
                self.slots[s].origin = origin;
            }
            g.add(s);
        }
        g.ana_wire(&mut self.slots);
        let slots = &mut self.slots;
        match kind {
            HorizontalDipole => g.set_width(slots, false, size(0.5, 1.0, rmd / 2.0)),
            VerticalDipole => g.set_height(slots, false, size(0.5, 1.0, rmd / 2.0)),
            VerticalDoubleSquare => g.set_perimeter(slots, false, size(1.5, 1.5 * 2.0, rmd * 1.5)),
            VerticalDoubleTriangle => {
                let d = size(0.71, 0.71 * 2.0, rmd * 0.71 * 2.0);
                g.set_width(slots, false, d);
                g.set_height(slots, false, d / 2.0);
            }
            _ => g.set_perimeter(slots, false, size(1.0, 2.0, rmd)),
        }
        if n < len {
            self.groups[n] = g;
        } else {
            self.groups.push(g);
        }
    }

    /// `AllRoundUp` (値を丸める) to multiples of 1/`sc`.
    fn round_values(&mut self, sc: f64, in_wavelengths: bool, in_millimetres: bool, all: bool) {
        let rmd = self.wavelength;
        // `GetRmdVal` / `SetRmdVal`
        let get = |d: f64| {
            if in_wavelengths {
                d / rmd
            } else if in_millimetres {
                d * 1000.0
            } else {
                d
            }
        };
        let set = |d: f64| {
            if in_wavelengths {
                d * rmd
            } else if in_millimetres {
                d / 1000.0
            } else {
                d
            }
        };
        for i in 0..self.groups.len() {
            let slots = &mut self.slots;
            let g = &mut self.groups[i];
            if g.xw {
                if g.width() != 0.0 {
                    let v = set(round_up(get(g.width()), sc));
                    g.set_width(slots, all, v);
                }
                if g.height() != 0.0 {
                    let v = set(round_up(get(g.height()), sc));
                    g.set_height(slots, all, v);
                }
                if g.perimeter() != 0.0 {
                    let v = set(round_up(get(g.perimeter()), sc));
                    g.set_perimeter(slots, all, v);
                }
            } else if g.perimeter() != 0.0 {
                let v = set(round_up(get(g.perimeter()), sc));
                g.set_perimeter(slots, all, v);
            }
            if self.position(i) != 0.0 {
                let v = set(round_up(get(self.position(i)), sc));
                self.set_position(i, v);
            }
            let g = &mut self.groups[i];
            let r = g.radius(&self.slots);
            if g.enb_r && r > 0.0 {
                let r = if in_wavelengths {
                    let r = set(round_up(get(r), sc));
                    if r == 0.0 { set(1.0 / sc) } else { r }
                } else {
                    // (the fallback is 1/sc metres in the original, not 1/sc mm)
                    let r = round_up(r, sc * 1000.0);
                    if r == 0.0 { 1.0 / sc * 1000.0 } else { r }
                };
                g.set_radius(&mut self.slots, r);
            }
        }
    }

    /// Applies one edit with the dialog's rules (`GridSetEditText`, the popup
    /// menu, `AllRoundUp`). Returns false when the edit is not allowed and
    /// nothing changed.
    ///
    /// The dialog accepts a width, height or perimeter of at least 0.001 in
    /// the unit it shows (metres or wavelengths). Values arrive here in
    /// metres, so that check belongs to the front end; this refuses sizes
    /// that are not positive.
    ///
    /// Unlike the original, it also refuses an edit that would leave a
    /// broken antenna (the original writes it back; [`apply_as_original`]
    /// does too):
    /// - sizes, positions or wires that are not finite (a width of 1e300
    ///   makes the perimeter infinite);
    /// - a resize, rounding, change of form or copy after which the table
    ///   no longer describes the element's wires where it did before: a
    ///   width that moves only the outermost points (no `scaleAll`) past
    ///   other points of the element shows the new width while the wires
    ///   keep the old one, the moved wires lying over the others;
    /// - a resize that moves only the outermost points along an axis the
    ///   table does not describe (a corner reflector taken for one square
    ///   shows half its width, and its other points would not move);
    /// - a wire shrunk to zero length, which no calculation accepts (rounding
    ///   a small element to whole metres, or reshaping a structure taken for
    ///   an octagon).
    ///
    /// [`apply_as_original`]: Self::apply_as_original
    pub fn apply(&mut self, op: &ElementOp) -> bool {
        use ElementOp::*;
        let before = self.clone();
        let zero_length = |list: &Self| list.slots.iter().filter(|s| s.live && s.wire.start == s.wire.end).count();
        let collapsed = zero_length(self);
        // the elements an edit reshapes, what of them the table described,
        // and their bounds
        let described = |list: &Self, row: usize| {
            let g = &list.groups[row];
            (g.describes_wires(&list.slots), (g.lo, g.hi))
        };
        let len = self.groups.len();
        let resized: Vec<(usize, Described)> = match *op {
            RoundValues { .. } => (0..len).map(|row| (row, described(self, row))).collect(),
            // (a rebuilt element is a template, which the table describes:
            // there is nothing for it to stop describing)
            // an edit in the new row first copies the last element, and a copy
            // is analysed again
            SetInterval { row, .. }
            | SetPosition { row, .. }
            | SetWidth { row, .. }
            | SetHeight { row, .. }
            | SetPerimeter { row, .. }
            | SetRadius { row, .. }
            | SetSegmentation { row, .. }
            | Insert { row }
                if row == len && len > 0 =>
            {
                vec![(row, described(self, len - 1))]
            }
            SetWidth { row, .. } | SetHeight { row, .. } | SetPerimeter { row, .. } | Insert { row } if row < len => {
                vec![(row, described(self, row))]
            }
            _ => Vec::new(),
        };
        // moving only the outermost points along an axis the table does not
        // describe moves points the table does not show
        let edge_only = matches!(
            *op,
            SetWidth { scale_all: false, .. }
                | SetHeight { scale_all: false, .. }
                | SetPerimeter { scale_all: false, .. }
                | RoundValues { scale_all: false, .. }
        );
        if !self.apply_as_original(op) {
            return false;
        }
        let broken = resized.iter().any(|&(row, (was, (lo, hi)))| {
            let (now, (nlo, nhi)) = described(self, row);
            let moved = |axis: usize| nlo[axis] != lo[axis] || nhi[axis] != hi[axis];
            was.iter().zip(&now).any(|(was, now)| *was && !now)
                || (edge_only && [X, Y, Z].into_iter().any(|axis| moved(axis) && !was[axis]))
        });
        if self.is_finite() && !broken && zero_length(self) <= collapsed {
            for &(row, _) in &resized {
                self.groups[row].follow_wires(&self.slots);
            }
            return true;
        }
        *self = before;
        false
    }

    /// Every size and position of the table and every wire it writes back is finite.
    fn is_finite(&self) -> bool {
        let rows = self
            .rows()
            .into_iter()
            .flat_map(|r| [r.interval, r.position, r.width, r.height, r.perimeter, r.radius.unwrap_or(0.0)]);
        let wires = self
            .wires()
            .wires
            .into_iter()
            .flat_map(|w| [w.start.x, w.start.y, w.start.z, w.end.x, w.end.y, w.end.z, w.radius]);
        rows.chain(wires).all(f64::is_finite)
    }

    /// [`apply`](Self::apply) with only the original's rules, which may
    /// leave infinite sizes, zero-length wires or a table that no longer
    /// describes the wires. `tests/elements.rs` compares it with MMANA.
    pub fn apply_as_original(&mut self, op: &ElementOp) -> bool {
        use ElementOp::*;
        let len = self.groups.len();
        // an edit in the "新規" row first creates the element (`GridNewLine`)
        let new_row = |list: &mut Self, row: usize, kind: ElementKind| {
            if row >= list.groups.len() {
                list.create(kind);
            }
        };
        match *op {
            RoundValues { decimals, in_wavelengths, in_millimetres, scale_all } => {
                if decimals > 9 || (in_wavelengths && self.wavelength == 0.0) {
                    return false;
                }
                self.round_values(10f64.powi(decimals as i32), in_wavelengths, in_millimetres, scale_all);
                return true;
            }
            SetInterval { row, .. }
            | SetPosition { row, .. }
            | SetWidth { row, .. }
            | SetHeight { row, .. }
            | SetPerimeter { row, .. }
            | SetRadius { row, .. }
            | SetSegmentation { row, .. }
            | SetKind { row, .. }
            | Insert { row }
                if row > len =>
            {
                return false;
            }
            _ => {}
        }
        match *op {
            SetInterval { row, value } => {
                if !value.is_finite() || value < 0.0 || row == self.base {
                    return false;
                }
                new_row(self, row, ElementKind::HorizontalDipole);
                *self.interval_mut(row) = value;
            }
            SetPosition { row, value } => {
                if !value.is_finite() || value < 0.0 || row == 0 {
                    return false;
                }
                new_row(self, row, ElementKind::HorizontalDipole);
                self.set_position(row, value);
            }
            SetWidth { row, value, scale_all } => {
                if !value.is_finite() || value <= 0.0 {
                    return false;
                }
                new_row(self, row, ElementKind::HorizontalDipole);
                self.groups[row].set_width(&mut self.slots, scale_all, value);
            }
            SetHeight { row, value, scale_all } => {
                if !value.is_finite() || value <= 0.0 {
                    return false;
                }
                new_row(self, row, ElementKind::VerticalDipole);
                self.groups[row].set_height(&mut self.slots, scale_all, value);
            }
            SetPerimeter { row, value, scale_all } => {
                if !value.is_finite() || value <= 0.0 {
                    return false;
                }
                new_row(self, row, ElementKind::VerticalSquare);
                self.groups[row].set_perimeter(&mut self.slots, scale_all, value);
            }
            SetRadius { row, value } => {
                if !value.is_finite() || (row < len && !self.groups[row].enb_r) {
                    return false;
                }
                new_row(self, row, ElementKind::HorizontalDipole);
                self.groups[row].set_radius(&mut self.slots, value);
            }
            SetSegmentation { row, segmentation } => {
                if row < len && !self.groups[row].enb_seg {
                    return false;
                }
                new_row(self, row, ElementKind::HorizontalDipole);
                let s = Segmentation::from_code(segmentation.to_code());
                self.groups[row].set_segmentation(&mut self.slots, s);
            }
            SetKind { row, kind } => {
                let template = kind.template();
                // (compares the template with the recognised form, as the original does)
                if !kind.is_template() || (row < len && self.groups[row].kind == template) {
                    return false;
                }
                self.set_kind(row, template);
            }
            Insert { row } => self.insert(row),
            Delete { row } => {
                if row >= len || row == self.base {
                    return false;
                }
                self.delete(row);
            }
            RoundValues { .. } => unreachable!(),
        }
        true
    }

    /// Two identical elements at the same place (`IsSameElement`), which the
    /// dialog refuses to write back: they would join into one when read again.
    pub fn duplicate(&self) -> Option<(usize, usize)> {
        let same = |i: usize, j: usize| self.groups[i].same_shape(&self.groups[j], &self.slots);
        for i in (1..=self.base).rev() {
            let mut j = i;
            while j > 0 && self.interval(j - 1) == 0.0 {
                j -= 1;
                if same(i, j) {
                    return Some((i, j));
                }
            }
        }
        for i in self.base..self.groups.len().saturating_sub(1) {
            let mut j = i + 1;
            while j < self.groups.len() && self.interval(j) == 0.0 {
                if same(i, j) {
                    return Some((i, j));
                }
                j += 1;
            }
        }
        None
    }

    /// The wires as the dialog writes them back (`Update`): each element
    /// placed at its position. When no wire was deleted every wire keeps its
    /// index (new ones follow); otherwise they are renumbered element by
    /// element, from the base element backwards and then forwards.
    pub fn wires(&self) -> EditedWires {
        let mut out = EditedWires { wires: Vec::new(), elements: Vec::new(), slots: Vec::new(), origins: Vec::new() };
        if self.groups.is_empty() {
            return out;
        }
        let renumber = self.slots.iter().any(|s| !s.live);
        let b = self.groups[self.base].c[X];
        let mut placed: Vec<Option<(Wire, usize)>> = vec![None; self.slots.len()];
        let mut order = Vec::new();
        let mut put = |i: usize, bx: f64, slots: &[Slot]| {
            for r in &self.groups[i].refs {
                let s = &slots[r.slot];
                if s.live {
                    let mut w = s.wire.clone();
                    w.start.x += bx + b;
                    w.end.x += bx + b;
                    placed[r.slot] = Some((w, i));
                    order.push(r.slot);
                }
            }
        };
        let mut bx = 0.0;
        for i in (0..=self.base).rev() {
            bx -= self.interval(i);
            put(i, bx, &self.slots);
        }
        let mut bx = 0.0;
        for i in self.base + 1..self.groups.len() {
            bx += self.interval(i);
            put(i, bx, &self.slots);
        }
        if !renumber {
            order = (0..self.slots.len()).filter(|&s| placed[s].is_some()).collect();
        }
        for s in order {
            let (w, e) = placed[s].take().expect("each wire is placed once");
            out.wires.push(w);
            out.elements.push(e);
            out.slots.push(s);
            out.origins.push(self.slots[s].origin);
        }
        out
    }
}

/// What the optimiser (`CACal`, see [`crate::optimize`]) does with the
/// element table. Like the original it changes wires and elements in place
/// and never analyses the elements again, so bounds stay as they were found.
impl ElementList {
    /// `wmax`
    pub(crate) fn wire_count(&self) -> usize {
        self.slots.len()
    }

    /// `wdef[w]`: X is relative to the element's centre (see [`Self::x_abs`]).
    pub(crate) fn wire(&self, w: usize) -> &Wire {
        &self.slots[w].wire
    }

    pub(crate) fn wire_mut(&mut self, w: usize) -> &mut Wire {
        &mut self.slots[w].wire
    }

    /// `GetEleNo`
    pub(crate) fn element_of(&self, w: usize) -> Option<usize> {
        self.groups.iter().position(|g| g.refs.iter().any(|r| r.slot == w))
    }

    /// `GetXAbs`: what a wire's relative X is added to. It counts from where
    /// the first element was found, so it is off once that element moved.
    pub(crate) fn x_abs(&self, w: usize) -> f64 {
        match self.element_of(w) {
            Some(e) => self.position(e) + self.groups[0].c[X],
            None => 0.0,
        }
    }

    /// `CACal::AdjWireChen`: the other wires of the element that ended where
    /// wire `w` ended (`old`) follow it.
    pub(crate) fn follow_wire(&mut self, w: usize, old: &Wire) {
        let Some(e) = self.element_of(w) else { return };
        let new = self.slots[w].wire.clone();
        for r in &self.groups[e].refs {
            if r.slot == w {
                continue;
            }
            let wp = &mut self.slots[r.slot].wire;
            for (o, n) in [(old.start, new.start), (old.end, new.end)] {
                if n != o {
                    // `AdjWireChen` moves one end at most
                    if wp.start == o {
                        wp.start = n;
                    } else if wp.end == o {
                        wp.end = n;
                    }
                }
            }
        }
    }

    pub(crate) fn element_interval(&self, n: usize) -> f64 {
        self.interval(n)
    }

    pub(crate) fn set_element_interval(&mut self, n: usize, d: f64) {
        *self.interval_mut(n) = d;
    }

    /// `GetXPosition`
    pub(crate) fn element_position(&self, n: usize) -> f64 {
        self.position(n)
    }

    /// `SetXPosition`
    pub(crate) fn set_element_position(&mut self, n: usize, d: f64) {
        self.set_position(n, d);
    }

    /// `GetWidth`, `GetHeight`, `GetRound`, `GetR` and `EnbXw` of element `n`.
    pub(crate) fn element_width(&self, n: usize) -> f64 {
        self.groups[n].width()
    }

    pub(crate) fn element_height(&self, n: usize) -> f64 {
        self.groups[n].height()
    }

    pub(crate) fn element_perimeter(&self, n: usize) -> f64 {
        self.groups[n].perimeter()
    }

    pub(crate) fn element_radius(&self, n: usize) -> f64 {
        self.groups[n].radius(&self.slots)
    }

    pub(crate) fn element_x_width(&self, n: usize) -> bool {
        self.groups[n].xw
    }

    /// `SetWidth` / `SetWidthAll`
    pub(crate) fn set_element_width(&mut self, n: usize, all: bool, d: f64) {
        self.groups[n].set_width(&mut self.slots, all, d);
    }

    /// `SetHeight` / `SetHeightAll`
    pub(crate) fn set_element_height(&mut self, n: usize, all: bool, d: f64) {
        self.groups[n].set_height(&mut self.slots, all, d);
    }

    /// `SetRound` / `SetRoundAll`
    pub(crate) fn set_element_perimeter(&mut self, n: usize, all: bool, d: f64) {
        self.groups[n].set_perimeter(&mut self.slots, all, d);
    }

    /// `SetR`
    pub(crate) fn set_element_radius(&mut self, n: usize, d: f64) {
        self.groups[n].set_radius(&mut self.slots, d);
    }

    /// For each element, along which axes its sizes are those of its wires
    /// (see [`ElementList::apply`]).
    pub(crate) fn described(&self) -> Vec<[bool; 4]> {
        self.groups.iter().map(|g| g.describes_wires(&self.slots)).collect()
    }

    /// For each element, which rule `new_bounds` takes along Y and Z: from
    /// 0, to 0, in proportion across 0, or about the centre.
    pub(crate) fn bound_rules(&self) -> Vec<[u8; 2]> {
        let rule = |g: &Group, axis: usize| match (g.lo[axis], g.hi[axis]) {
            (0.0, _) => 0,
            (_, 0.0) => 1,
            (lo, hi) if lo < 0.0 && hi > 0.0 => 2,
            _ => 3,
        };
        self.groups.iter().map(|g| [rule(g, Y), rule(g, Z)]).collect()
    }
}

/// Result of [`edit_antenna`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ElementEdit {
    /// The element table after the edits.
    pub elements: Vec<ElementRow>,
    /// The element intervals count from.
    pub base: usize,
    /// The antenna with the edited wires. Sources and loads follow their
    /// wires; those on deleted wires are removed. Unchanged when no edit applied.
    pub antenna: Antenna,
    /// Element (row) of each wire of `antenna`.
    pub wire_elements: Vec<usize>,
    /// Whether each operation was applied (false: not allowed, nothing changed).
    pub applied: Vec<bool>,
    /// Two identical elements at the same place (0-based rows). MMANA does
    /// not write the wires back in this state.
    pub duplicate: Option<[usize; 2]>,
    pub warnings: Vec<String>,
}

/// Opens the element editor on `antenna`, applies `ops` in order and writes
/// the wires back, as MMANA's element dialog does on OK.
pub fn edit_antenna(antenna: &Antenna, ops: &[ElementOp]) -> ElementEdit {
    let f = antenna.frequency_mhz;
    let wavelength = if f != 0.0 { SPEED_OF_LIGHT_MMANA / f } else { 0.0 };
    let mut list = ElementList::new(&antenna.wires, wavelength);
    let applied: Vec<bool> = ops.iter().map(|op| list.apply(op)).collect();
    let duplicate = list.duplicate().map(|(a, b)| [a, b]);
    let mut warnings = Vec::new();
    let mut edited = antenna.clone();
    let wire_elements;
    if applied.contains(&true) {
        let out = list.wires();
        // new index of each original wire, if it still exists
        let mut moved: Vec<Option<usize>> = vec![None; antenna.wires.len()];
        for (i, o) in out.origins.iter().enumerate() {
            if let Some(o) = *o {
                moved[o] = Some(i);
            }
        }
        edited.wires = out.wires;
        wire_elements = out.elements;
        let sources = edited.sources.len();
        edited.sources.retain_mut(|s| follow_wire(&mut s.pulse, &moved, "source", &mut warnings));
        if edited.sources.len() != sources {
            set_auto_voltage(&mut edited);
        }
        edited.loads.retain_mut(|l| follow_wire(&mut l.pulse, &moved, "load", &mut warnings));
    } else {
        // nothing changed: the wires are not rewritten (that would round X)
        let mut of_slot = vec![0; antenna.wires.len()];
        for (row, g) in list.groups.iter().enumerate() {
            for r in &g.refs {
                of_slot[r.slot] = row;
            }
        }
        wire_elements = of_slot;
    }
    ElementEdit {
        elements: list.rows(),
        base: list.base(),
        antenna: edited,
        wire_elements,
        applied,
        duplicate,
        warnings,
    }
}

/// `SetAutoVol`: with automatic voltage on, every source gets 1 / count.
/// MMANA runs it whenever a source is added or removed.
fn set_auto_voltage(antenna: &mut Antenna) {
    if antenna.auto_voltage {
        let v = 1.0 / antenna.sources.len() as f64;
        for s in &mut antenna.sources {
            s.voltage = v;
        }
    }
}

/// Renumbers the wire of a pulse name (`w3c` → `w5c`). False when the wire
/// was deleted. Raw pulse numbers cannot follow and are kept with a warning.
fn follow_wire(pulse: &mut String, moved: &[Option<usize>], what: &str, warnings: &mut Vec<String>) -> bool {
    let name = to_half_width(pulse);
    let name = name.trim();
    let Some(rest) = name.strip_prefix(['w', 'W']) else {
        if !moved.is_empty() {
            warnings.push(format!("{what} '{pulse}' is a pulse number and may now refer to another wire"));
        }
        return true;
    };
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    let Ok(n) = rest[..digits].parse::<usize>() else { return true };
    if n == 0 || n > moved.len() {
        return true;
    }
    match moved[n - 1] {
        Some(i) => {
            *pulse = format!("{}{}{}", &name[..1], i + 1, &rest[digits..]);
            true
        }
        None => {
            warnings.push(format!("{what} '{pulse}' was removed with its wire"));
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wire(a: (f64, f64, f64), b: (f64, f64, f64)) -> Wire {
        Wire {
            start: Point3::new(a.0, a.1, a.2),
            end: Point3::new(b.0, b.1, b.2),
            radius: 0.001,
            segmentation: Segmentation::Taper,
            note: String::new(),
            group: String::new(),
        }
    }

    /// A 3-element Yagi with a quad loop as the driven element at X = 0.
    fn yagi_with_quad() -> Vec<Wire> {
        vec![
            wire((-2.0, -5.0, 0.0), (-2.0, 5.0, 0.0)),
            wire((0.0, -2.5, -2.5), (0.0, 2.5, -2.5)),
            wire((0.0, 2.5, -2.5), (0.0, 2.5, 2.5)),
            wire((0.0, 2.5, 2.5), (0.0, -2.5, 2.5)),
            wire((0.0, -2.5, 2.5), (0.0, -2.5, -2.5)),
            wire((3.0, -4.5, 0.0), (3.0, 4.5, 0.0)),
        ]
    }

    #[test]
    fn recognises_elements_and_spacing() {
        let list = ElementList::new(&yagi_with_quad(), 21.3);
        let rows = list.rows();
        assert_eq!(rows.len(), 3);
        assert_eq!(list.base(), 1);
        assert_eq!(rows[0].kind, ElementKind::HorizontalDipole);
        assert_eq!(rows[1].kind, ElementKind::VerticalSquare);
        assert_eq!((rows[1].width, rows[1].height, rows[1].perimeter), (5.0, 5.0, 20.0));
        assert_eq!(rows[1].wire_count, 4);
        assert_eq!([rows[0].interval, rows[1].interval, rows[2].interval], [2.0, 0.0, 3.0]);
        assert_eq!([rows[0].position, rows[1].position, rows[2].position], [0.0, 2.0, 5.0]);
        assert_eq!(rows[0].width, 10.0);
    }

    #[test]
    fn unchanged_antenna_is_not_rewritten() {
        let mut ant = Antenna { wires: yagi_with_quad(), ..Antenna::default() };
        ant.wires[0].start.x = 0.1 + 0.2 - 2.3; // a value that would not survive X - c + c
        ant.wires[0].end.x = ant.wires[0].start.x;
        let e = edit_antenna(&ant, &[ElementOp::Delete { row: 1 }]);
        assert_eq!(e.applied, vec![false]);
        assert_eq!(e.antenna, ant);
        assert_eq!(e.wire_elements, vec![0, 1, 1, 1, 1, 2]);
    }

    #[test]
    fn perimeter_keeps_the_square() {
        let mut list = ElementList::new(&yagi_with_quad(), 21.3);
        assert!(list.apply(&ElementOp::SetPerimeter { row: 1, value: 24.0, scale_all: false }));
        let r = &list.rows()[1];
        assert_eq!((r.width, r.height, r.perimeter), (6.0, 6.0, 24.0));
    }

    #[test]
    fn a_stub_beyond_a_bound_moves_with_it_once() {
        // a square loop from -1 to 1 m with a 25 cm stub below its bottom
        // (the bounds are the loop's, as the skydoor sample's)
        let wires = vec![
            wire((0.0, -1.0, -1.0), (0.0, -0.1, -1.0)),
            wire((0.0, -0.1, -1.0), (0.0, 0.1, -1.0)),
            wire((0.0, 0.1, -1.0), (0.0, 1.0, -1.0)),
            wire((0.0, -1.0, 1.0), (0.0, 1.0, 1.0)),
            wire((0.0, -1.0, -1.0), (0.0, -1.0, 1.0)),
            wire((0.0, 1.0, -1.0), (0.0, 1.0, 1.0)),
            wire((0.0, -0.1, -1.0), (0.0, -0.1, -1.25)),
            wire((0.0, 0.1, -1.0), (0.0, 0.1, -1.25)),
            wire((0.0, -0.1, -1.25), (0.0, 0.1, -1.25)),
        ];
        let mut list = ElementList::new(&wires, 21.3);
        assert_eq!((list.groups[0].lo[Z], list.groups[0].hi[Z]), (-1.0, 1.0));
        // the bottom 25 cm higher: the stub moves with it, its end onto the old
        // bottom, and stays there
        assert!(list.apply_as_original(&ElementOp::SetHeight { row: 0, value: 1.5, scale_all: false }));
        let out = list.wires().wires;
        let bars: Vec<_> =
            out.iter().filter(|w| w.start.y == -0.1 && w.end.y == 0.1).map(|w| (w.start.z, w.end.z)).collect();
        assert_eq!(bars.len(), 2);
        assert!(bars.contains(&(-0.75, -0.75)) && bars.contains(&(-1.0, -1.0)), "{bars:?}");
        assert!(out.iter().all(|w| w.start != w.end));
    }

    #[test]
    fn deleting_renumbers_wires_and_pulses() {
        let mut ant = Antenna { wires: yagi_with_quad(), ..Antenna::default() };
        ant.sources = vec![crate::Source { pulse: "w3c".into(), phase_deg: 0.0, voltage: 1.0 }];
        ant.loads =
            vec![crate::Load { pulse: "ｗ６ｂ".into(), kind: crate::LoadKind::Impedance { r: 1.0, x: 0.0 } }];
        let e = edit_antenna(&ant, &[ElementOp::Delete { row: 2 }]);
        assert_eq!(e.applied, vec![true]);
        assert_eq!(e.antenna.wires.len(), 5);
        // renumbered from the base element backwards: the quad, then the reflector
        assert_eq!(e.antenna.wires[4], ant.wires[0]);
        assert_eq!(e.wire_elements, vec![1, 1, 1, 1, 0]);
        assert_eq!(e.antenna.sources[0].pulse, "w2c");
        assert!(e.antenna.loads.is_empty());
        assert_eq!(e.warnings.len(), 1);
    }

    #[test]
    fn removing_a_source_resets_automatic_voltages() {
        let source = |pulse: &str| crate::Source { pulse: pulse.into(), phase_deg: 0.0, voltage: 0.5 };
        for auto_voltage in [true, false] {
            let ant = Antenna {
                wires: yagi_with_quad(),
                sources: vec![source("w3c"), source("w6c")],
                auto_voltage,
                ..Antenna::default()
            };
            let e = edit_antenna(&ant, &[ElementOp::Delete { row: 2 }]);
            assert_eq!(e.antenna.sources.len(), 1);
            assert_eq!(e.antenna.sources[0].voltage, if auto_voltage { 1.0 } else { 0.5 });
        }
    }

    #[test]
    fn pulses_do_not_move_to_a_wire_that_reuses_a_deleted_slot() {
        let ant = Antenna {
            wires: vec![wire((0.0, -1.0, 0.0), (0.0, 1.0, 0.0)), wire((3.0, -1.0, 0.0), (3.0, 1.0, 0.0))],
            sources: vec![crate::Source { pulse: "w2c".into(), phase_deg: 0.0, voltage: 1.0 }],
            ..Antenna::default()
        };
        // the copy of element 1 takes the slot wire 2 left
        let e = edit_antenna(&ant, &[ElementOp::Delete { row: 1 }, ElementOp::Insert { row: 0 }]);
        assert_eq!(e.applied, vec![true, true]);
        assert_eq!(e.antenna.wires.len(), 2);
        assert!(e.antenna.sources.is_empty());
        assert_eq!(e.warnings.len(), 1);
    }

    #[test]
    fn pulses_stay_on_a_rebuilt_element() {
        let mut ant = Antenna { wires: yagi_with_quad(), ..Antenna::default() };
        ant.sources = vec![crate::Source { pulse: "w2c".into(), phase_deg: 0.0, voltage: 1.0 }];
        ant.loads = vec![crate::Load { pulse: "w4c".into(), kind: crate::LoadKind::Impedance { r: 1.0, x: 0.0 } }];
        // deleting the reflector frees slot 0, which the dipole replacing the
        // quad then takes; the source on the quad's first wire goes with it
        let e = edit_antenna(
            &ant,
            &[ElementOp::Delete { row: 0 }, ElementOp::SetKind { row: 0, kind: ElementKind::HorizontalDipole }],
        );
        assert_eq!(e.applied, vec![true, true]);
        assert_eq!(e.antenna.wires.len(), 2);
        assert_eq!(e.antenna.sources.len(), 1);
        let pulse = &e.antenna.sources[0].pulse;
        let w: usize = pulse[1..pulse.len() - 1].parse().unwrap();
        assert_eq!(e.elements[e.wire_elements[w - 1]].kind, ElementKind::HorizontalDipole);
        // the dipole has one wire: the load on the quad's third wire is gone
        assert!(e.antenna.loads.is_empty());
        assert_eq!(e.warnings, vec!["load 'w4c' was removed with its wire".to_string()]);
    }

    #[test]
    fn sizes_below_a_millimetre_are_allowed() {
        // 0.002 λ at 1296 MHz, which MMANA accepts in wavelength display
        let mut list = ElementList::new(&yagi_with_quad(), 299.8 / 1296.0);
        let w = 0.002 * (299.8 / 1296.0);
        assert!(w < 0.001);
        assert!(list.apply(&ElementOp::SetWidth { row: 0, value: w, scale_all: false }));
        assert_eq!(list.rows()[0].width, w);
        assert!(list.apply(&ElementOp::SetPerimeter { row: 1, value: 0.0005, scale_all: true }));
        assert!(!list.apply(&ElementOp::SetHeight { row: 1, value: 0.0, scale_all: false }));
        assert!(!list.apply(&ElementOp::SetWidth { row: 1, value: -1.0, scale_all: false }));
    }

    #[test]
    fn base_element_cannot_be_deleted() {
        let mut list = ElementList::new(&yagi_with_quad(), 21.3);
        assert!(!list.apply(&ElementOp::Delete { row: 1 }));
        assert!(!list.apply(&ElementOp::SetInterval { row: 1, value: 1.0 }));
        assert!(!list.apply(&ElementOp::SetWidth { row: 9, value: 1.0, scale_all: false }));
        assert!(!list.apply(&ElementOp::SetWidth { row: 0, value: f64::NAN, scale_all: false }));
    }

    #[test]
    fn new_row_creates_a_copy_and_duplicates_are_found() {
        let mut list = ElementList::new(&yagi_with_quad(), 21.3);
        assert!(list.apply(&ElementOp::SetWidth { row: 3, value: 8.0, scale_all: false }));
        let rows = list.rows();
        assert_eq!(rows.len(), 4);
        assert_eq!((rows[3].kind, rows[3].width, rows[3].interval), (ElementKind::HorizontalDipole, 8.0, 3.0));
        assert_eq!(list.duplicate(), None);
        assert!(list.apply(&ElementOp::Insert { row: 3 }));
        assert!(list.apply(&ElementOp::SetInterval { row: 4, value: 0.0 }));
        assert_eq!(list.duplicate(), Some((3, 4)));
        // the copies are new wires: the sources and loads stay on the originals
        let out = list.wires();
        let copies = out.elements.iter().zip(&out.origins).filter(|(e, _)| **e >= 3);
        assert!(copies.clone().count() > 0 && copies.clone().all(|(_, o)| o.is_none()), "{:?}", out.origins);
    }

    #[test]
    fn duplicates_are_found_before_the_base_element() {
        // two reflectors behind the element at X = 0, the second made the same
        // as the first and moved onto it
        let wires = [
            wire((-6.0, -4.0, 0.0), (-6.0, 4.0, 0.0)),
            wire((-3.0, -5.0, 0.0), (-3.0, 5.0, 0.0)),
            wire((0.0, -5.0, 0.0), (0.0, 5.0, 0.0)),
        ];
        let mut list = ElementList::new(&wires, 21.3);
        assert_eq!(list.base, 2);
        assert!(list.apply(&ElementOp::SetWidth { row: 0, value: 10.0, scale_all: false }));
        assert_eq!(list.duplicate(), None);
        assert!(list.apply(&ElementOp::SetInterval { row: 0, value: 0.0 }));
        assert_eq!(list.duplicate(), Some((1, 0)));
    }

    #[test]
    fn elements_sharing_one_end_are_not_the_same() {
        // a 5 m vertical moved onto the 10 m one: one end in common
        let wires = [wire((0.0, 0.0, 0.0), (0.0, 0.0, 10.0)), wire((2.0, 0.0, 0.0), (2.0, 0.0, 5.0))];
        let mut list = ElementList::new(&wires, 21.3);
        assert!(list.apply_as_original(&ElementOp::SetInterval { row: 1, value: 0.0 }));
        assert_eq!(list.duplicate(), None);
    }

    #[test]
    fn an_element_of_no_size_is_rebuilt_a_wavelength_across() {
        // a wire of no length has no perimeter, width or height: the double
        // triangle takes 1.42 λ
        let mut list = ElementList::new(&[wire((0.0, 1.0, 2.0), (0.0, 1.0, 2.0))], 20.0);
        assert!(list.apply(&ElementOp::SetKind { row: 0, kind: ElementKind::VerticalDoubleTriangle }));
        let row = &list.rows()[0];
        assert!((row.width - 28.4).abs() < 1e-9 && (row.height - 14.2).abs() < 1e-9, "{row:?}");
    }

    #[test]
    fn the_editor_takes_the_wavelength_of_the_antenna() {
        // a new loop is a wavelength around
        let ant =
            Antenna { frequency_mhz: 14.0, wires: vec![wire((0.0, -5.0, 0.0), (0.0, 5.0, 0.0))], ..Antenna::default() };
        let e = edit_antenna(&ant, &[ElementOp::SetKind { row: 1, kind: ElementKind::VerticalSquare }]);
        assert_eq!(e.applied, vec![true]);
        assert!((e.elements[1].perimeter - SPEED_OF_LIGHT_MMANA / 14.0).abs() < 1e-9, "{:?}", e.elements[1]);
    }

    #[test]
    fn empty_antenna_starts_in_the_new_row() {
        let mut list = ElementList::new(&[], 21.3);
        assert!(list.is_empty());
        assert!(!list.apply(&ElementOp::SetInterval { row: 0, value: 1.0 }));
        assert!(list.apply(&ElementOp::SetWidth { row: 0, value: 10.0, scale_all: false }));
        let rows = list.rows();
        assert_eq!((rows.len(), rows[0].kind, rows[0].width), (1, ElementKind::HorizontalDipole, 10.0));
        let out = list.wires();
        assert_eq!(out.wires.len(), 1);
        assert_eq!((out.wires[0].start.y, out.wires[0].end.y), (-5.0, 5.0));
    }

    #[test]
    fn templates_are_recognised_as_their_form() {
        use ElementKind::*;
        let cases = [
            (VerticalDipole, VerticalDipole),
            (VerticalSquare, VerticalSquare),
            (VerticalDiamond, VerticalDiamond),
            (VerticalTriangle, VerticalTriangle),
            (VerticalInvertedTriangle, VerticalInvertedTriangle),
            (VerticalOctagon, VerticalOctagon),
            (HorizontalSquare, HorizontalSquare),
            (HorizontalDiamond, HorizontalDiamond),
            (HorizontalTriangle, HorizontalTriangle),
            (HorizontalInvertedTriangle, HorizontalInvertedTriangle),
            (VerticalDoubleSquare, VerticalSquare),
            (VerticalDoubleTriangle, VerticalLoop),
        ];
        for (kind, recognised) in cases {
            let mut list = ElementList::new(&[wire((0.0, -5.0, 0.0), (0.0, 5.0, 0.0))], 21.3);
            assert!(list.apply(&ElementOp::SetKind { row: 0, kind }), "{kind:?}");
            assert_eq!(list.rows()[0].kind, recognised, "{kind:?}");
        }
        // a form is rebuilt only when it differs (by template, like the original)
        let mut list = ElementList::new(&yagi_with_quad(), 21.3);
        assert!(!list.apply(&ElementOp::SetKind { row: 1, kind: VerticalSquare }));
        assert!(!list.apply(&ElementOp::SetKind { row: 1, kind: VerticalLoop }));
    }

    #[test]
    fn round_up_matches_the_original() {
        assert_eq!(round_up(1.23456, 1000.0), 1.235);
        assert_eq!(round_up(-1.2345, 100.0), -1.23);
        assert_eq!(round_up(0.25, 10.0), 0.3);
        // the original's int() overflows here (a 350 m wire rounded to 0.0001 mm)
        assert_eq!(round_up(350034.98082, 10000.0), 350034.9808);
    }

    fn sample(name: &str) -> Vec<Wire> {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/maa/");
        crate::maa::parse_bytes(&std::fs::read(format!("{path}{name}")).unwrap()).project.antenna.wires
    }

    fn finite(list: &ElementList) -> bool {
        list.rows()
            .iter()
            .all(|r| [r.interval, r.position, r.width, r.height, r.perimeter].iter().all(|c| c.is_finite()))
    }

    #[test]
    fn elements_made_in_the_new_row_can_be_deleted() {
        // the original's fixed interval array had room for them; ours had not
        let mut list = ElementList::new(&[wire((0.0, -5.0, 0.0), (0.0, 5.0, 0.0))], 21.3);
        assert!(list.apply(&ElementOp::SetKind { row: 1, kind: ElementKind::VerticalSquare }));
        assert!(list.apply(&ElementOp::SetKind { row: 2, kind: ElementKind::VerticalSquare }));
        assert!(list.apply(&ElementOp::Delete { row: 1 }));
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn a_closed_chain_that_never_turns_has_finite_sizes() {
        // a wire and its reverse close a loop without corners; the original
        // shows sizes of -inf
        let list =
            ElementList::new(&[wire((0.0, -5.0, 0.0), (0.0, 5.0, 0.0)), wire((0.0, 5.0, 0.0), (0.0, -5.0, 0.0))], 21.3);
        assert!(finite(&list));
        assert_eq!(list.rows()[0].width, 10.0);
    }

    #[test]
    fn edits_that_overflow_are_refused() {
        let mut list = ElementList::new(&yagi_with_quad(), 21.3);
        let before = list.rows();
        // (the perimeter of the square would be 4 × f64::MAX)
        assert!(!list.apply(&ElementOp::SetWidth { row: 1, value: f64::MAX, scale_all: false }));
        assert_eq!(list.rows(), before);
        // the original writes the infinities back
        assert!(list.apply_as_original(&ElementOp::SetWidth { row: 1, value: f64::MAX, scale_all: false }));
        assert!(!finite(&list));
    }

    #[test]
    fn a_width_that_moves_the_tips_past_the_rest_is_refused() {
        // a 20 m dipole of 6.52 m with tips out to 8.38 m
        let wires = vec![
            wire((0.0, 3.26, 0.0), (0.0, -3.26, 0.0)),
            wire((0.0, 3.26, 0.0), (0.0, 4.19, 0.0)),
            wire((0.0, -3.26, 0.0), (0.0, -4.19, 0.0)),
        ];
        let mut list = ElementList::new(&wires, 21.3);
        // the tips would end inside the dipole, lying over it, and the table
        // would show 5 m for an antenna 6.52 m wide
        assert!(!list.apply(&ElementOp::SetWidth { row: 0, value: 5.0, scale_all: false }));
        assert!(list.clone().apply_as_original(&ElementOp::SetWidth { row: 0, value: 5.0, scale_all: false }));
        // shorter tips, or everything scaled, are fine
        assert!(list.apply(&ElementOp::SetWidth { row: 0, value: 7.0, scale_all: false }));
        assert!(list.apply(&ElementOp::SetWidth { row: 0, value: 5.0, scale_all: true }));
        let ends = list.wires().wires.iter().flat_map(|w| [w.start.y, w.end.y]).fold(0.0, |m: f64, y| m.max(y));
        assert!((ends - 2.5).abs() < 1e-12 && list.rows()[0].width == 5.0);
    }

    #[test]
    fn rounding_that_shrinks_a_wire_to_nothing_is_refused() {
        // a 4 mm stub at the end of a wire from 0, rounded to centimetres
        let wires = vec![wire((0.0, 0.0, 0.0), (0.0, 1.0, 0.0)), wire((0.0, 1.0, 0.0), (0.0, 1.004, 0.0))];
        let round = |decimals| ElementOp::RoundValues {
            decimals,
            in_wavelengths: false,
            in_millimetres: false,
            scale_all: false,
        };
        let mut list = ElementList::new(&wires, 21.3);
        assert!(!list.apply(&round(2)));
        assert!(list.clone().apply_as_original(&round(2)));
        assert!(list.apply(&round(3)));
    }

    #[test]
    fn scaling_an_element_off_0_keeps_later_edits_on_its_wires() {
        // a vertical dipole from 2 m to 12 m: scaling every coordinate scales
        // about 0, so it moves to 4 m to 24 m while its bounds stay centred
        // at 7 m; the next edit of its ends must still find them
        let mut list = ElementList::new(&[wire((0.0, 0.0, 2.0), (0.0, 0.0, 12.0))], 21.3);
        assert!(list.apply(&ElementOp::SetHeight { row: 0, value: 20.0, scale_all: true }));
        assert!(list.apply(&ElementOp::SetHeight { row: 0, value: 10.0, scale_all: false }));
        let w = &list.wires().wires[0];
        assert_eq!((w.start.z, w.end.z), (9.0, 19.0));
        assert_eq!(list.rows()[0].height, 10.0);
    }

    #[test]
    fn edge_edits_of_an_element_the_table_does_not_describe_are_refused() {
        // the corner reflector's grid is taken for one square half its width
        let mut list = ElementList::new(&sample("CR2.MAA"), 299.8 / 145.0);
        let row = (0..list.len()).find(|&r| list.rows()[r].wire_count > 1).unwrap();
        let width = list.rows()[row].width;
        assert!(!list.apply(&ElementOp::SetWidth { row, value: width * 0.9, scale_all: false }));
        // scaling every coordinate moves every point, so that one is fine
        assert!(list.apply(&ElementOp::SetWidth { row, value: width * 0.9, scale_all: true }));
    }

    #[test]
    fn templates_are_tapered_loops_sized_from_the_old_element() {
        use ElementKind::*;
        for kind in [
            VerticalSquare,
            VerticalDiamond,
            VerticalTriangle,
            VerticalInvertedTriangle,
            VerticalOctagon,
            HorizontalSquare,
            HorizontalDiamond,
            HorizontalTriangle,
            HorizontalInvertedTriangle,
            VerticalDoubleSquare,
            VerticalDoubleTriangle,
        ] {
            let mut list = ElementList::new(&[wire((0.0, -5.0, 0.0), (0.0, 5.0, 0.0))], 21.3);
            assert!(list.apply(&ElementOp::SetKind { row: 0, kind }), "{kind:?}");
            assert!(list.wires().wires.iter().all(|w| w.segmentation == Segmentation::Taper), "{kind:?}");
        }
        // a dipole has no perimeter: the double triangle takes 1.42 times its width
        let mut list = ElementList::new(&[wire((0.0, -5.0, 0.0), (0.0, 5.0, 0.0))], 21.3);
        assert!(list.apply(&ElementOp::SetKind { row: 0, kind: VerticalDoubleTriangle }));
        let row = &list.rows()[0];
        assert!((row.width - 14.2).abs() < 1e-9 && (row.height - 7.1).abs() < 1e-9, "{row:?}");
    }

    #[test]
    fn moving_the_last_element_leaves_no_interval_beyond_it() {
        let mut list = ElementList::new(&yagi_with_quad(), 21.3);
        assert!(list.apply(&ElementOp::SetPosition { row: 2, value: 6.0 }));
        assert_eq!(list.rows().iter().map(|r| r.position).collect::<Vec<_>>(), [0.0, 2.0, 6.0]);
        assert_eq!(list.intervals.len(), 3);
    }

    #[test]
    fn edits_at_the_limits_of_the_table() {
        let round = |decimals| ElementOp::RoundValues {
            decimals,
            in_wavelengths: false,
            in_millimetres: false,
            scale_all: false,
        };
        let mut list = ElementList::new(&yagi_with_quad(), 21.3);
        assert!(list.apply_as_original(&round(9)));
        assert!(!list.apply_as_original(&round(10)));
        // the first element is where positions are measured from
        assert!(!list.apply_as_original(&ElementOp::SetPosition { row: 0, value: 1.0 }));
        assert!(!list.apply_as_original(&ElementOp::SetPosition { row: 2, value: -1.0 }));
        assert!(list.apply_as_original(&ElementOp::SetPosition { row: 2, value: 1.0 }));
    }

    #[test]
    fn rounding_leaves_a_wire_without_a_radius_without_one() {
        // R = 0 marks a wire left out of the calculation
        let mut w = wire((0.0, -5.0, 0.0), (0.0, 5.0, 0.0));
        w.radius = 0.0;
        for in_wavelengths in [false, true] {
            let mut list = ElementList::new(std::slice::from_ref(&w), 21.3);
            let op = ElementOp::RoundValues { decimals: 2, in_wavelengths, in_millimetres: false, scale_all: false };
            assert!(list.apply_as_original(&op));
            assert_eq!(list.wires().wires[0].radius, 0.0, "in wavelengths: {in_wavelengths}");
        }
    }

    #[test]
    fn wires_are_compared_to_10_ppm_of_the_element() {
        let near = |half: f64| {
            let list = ElementList::new(&[wire((0.0, -half, 0.0), (0.0, half, 0.0))], 21.3);
            list.groups[0].extent(&list.slots).1
        };
        let large = near(10.0);
        assert!(large(0.0, 1.9e-4) && !large(0.0, 2.1e-4));
        // at least 0.01 mm
        let small = near(0.1);
        assert!(small(0.0, 0.9e-5) && !small(0.0, 1.1e-5));
    }
}
