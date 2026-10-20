//! Reading and writing MMANA `.maa` antenna files.
//!
//! The reader follows `TMainWnd::LoadAntStrings` field by field (including its
//! tolerance for missing trailing sections); the writer follows
//! `SaveAntStrings`, so files round-trip with the original program.

use crate::expr::{ExprContext, eval, eval_int, eval_uint};
use crate::format::{str_dbl, str_dbl_e};
use crate::model::*;

const WIRE_NOTE_BEGIN: &str = "[MMANA-WIRE-NOTES]";
const WIRE_NOTE_END: &str = "[/MMANA-WIRE-NOTES]";

/// Result of reading a file: the project plus non-fatal issues found.
#[derive(Debug, Clone)]
pub struct ParsedMaa {
    pub project: Project,
    pub warnings: Vec<String>,
}

/// Decodes raw file bytes. MMANA writes Shift_JIS (CP932); UTF-8 is accepted
/// for files edited with modern tools.
pub fn decode(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_owned(),
        Err(_) => encoding_rs::SHIFT_JIS.decode(bytes).0.into_owned(),
    }
}

/// Encodes text the way MMANA saves it (Shift_JIS).
pub fn encode(text: &str) -> Vec<u8> {
    encoding_rs::SHIFT_JIS.encode(text).0.into_owned()
}

pub fn parse_bytes(bytes: &[u8]) -> ParsedMaa {
    parse(&decode(bytes))
}

/// Splits on `,` and skips leading blanks like `StrDlm`.
fn fields(line: &str) -> Vec<&str> {
    line.split(',').map(|f| f.trim_start_matches([' ', '\t'])).collect()
}

struct Lines<'a> {
    iter: std::str::Lines<'a>,
}

impl<'a> Lines<'a> {
    /// `LoadText`: the next raw line without CR / EOF markers.
    fn raw(&mut self) -> Option<String> {
        self.iter.next().map(|l| l.chars().filter(|&c| c != '\r' && c != '\u{1a}').collect())
    }

    /// `LoadTextLoop`: the next line that does not start with `*`.
    fn data(&mut self) -> Option<String> {
        loop {
            let l = self.raw()?;
            if !l.starts_with('*') {
                return Some(l);
            }
        }
    }
}

pub fn parse(text: &str) -> ParsedMaa {
    let mut p = Parser {
        lines: Lines { iter: text.lines() },
        ctx: ExprContext { wavelength: SPEED_OF_LIGHT_MMANA / 14.05 },
        project: Project::default(),
        warnings: Vec::new(),
    };
    if p.read().is_none() {
        p.warnings.push("file ended before all sections were read".into());
    }
    finish_comment(&mut p.project.antenna);
    ParsedMaa { project: p.project, warnings: p.warnings }
}

struct Parser<'a> {
    lines: Lines<'a>,
    ctx: ExprContext,
    project: Project,
    warnings: Vec<String>,
}

impl Parser<'_> {
    fn num(&self, s: &str) -> f64 {
        eval(s, &self.ctx).0
    }

    fn int(&self, s: &str) -> i32 {
        eval_int(s, &self.ctx).0
    }

    fn uint(&self, s: &str) -> i32 {
        eval_uint(s, &self.ctx).0
    }

    /// Returns `None` when the file ends early (what the original treats as
    /// "stop reading, keep what we have").
    fn read(&mut self) -> Option<()> {
        let name = self.lines.data()?;
        self.project.antenna.name = name;

        let fq = self.lines.data()?;
        let fq = self.num(&fq);
        self.project.antenna.frequency_mhz = fq;
        if fq > 0.0 {
            self.ctx.wavelength = SPEED_OF_LIGHT_MMANA / fq;
        }

        self.read_wires()?;
        self.read_sources()?;
        self.read_loads()?;
        self.read_segmentation_and_environment()?;

        while let Some(line) = self.lines.data() {
            if line.starts_with("$$$") {
                self.read_combined_wires()?;
            } else if line.starts_with("###") {
                let mut comment = String::new();
                while let Some(l) = self.lines.raw() {
                    comment.push_str(&l);
                    comment.push_str("\r\n");
                }
                self.project.antenna.comment = comment;
            }
        }
        Some(())
    }

    fn read_wires(&mut self) -> Option<()> {
        let line = self.lines.data()?;
        let f = fields(&line);
        let count = self.uint(f[0]).max(0) as usize;
        let ant = &mut self.project.antenna;
        if f.get(1).is_some_and(|s| !s.is_empty()) {
            let get = |i: usize| f.get(i).copied().unwrap_or("");
            ant.stack.horizontal = eval_uint(get(1), &self.ctx).0;
            ant.stack.vertical = eval_uint(get(2), &self.ctx).0;
            ant.stack.horizontal_spacing = eval(get(3), &self.ctx).0;
            ant.stack.vertical_spacing = eval(get(4), &self.ctx).0;
            ant.stack.direction = match eval_uint(get(5), &self.ctx).0 {
                1 => StackDirection::Up,
                2 => StackDirection::Down,
                _ => StackDirection::Centered,
            };
            if ant.stack.horizontal < 1 || ant.stack.vertical < 1 {
                ant.stack.horizontal = 1;
                ant.stack.vertical = 1;
            }
        } else {
            ant.stack.direction = StackDirection::Centered;
            ant.stack.horizontal = 1;
            ant.stack.vertical = 1;
        }
        if !ant.stack.is_stacked() && ant.frequency_mhz != 0.0 {
            let w = SPEED_OF_LIGHT_MMANA / ant.frequency_mhz;
            ant.stack.horizontal_spacing = w;
            ant.stack.vertical_spacing = w;
        }

        for _ in 0..count {
            let Some(line) = self.lines.data() else {
                self.warnings.push(format!("expected {count} wires, found {}", self.project.antenna.wires.len()));
                return None;
            };
            let f = fields(&line);
            let get = |i: usize| f.get(i).copied().unwrap_or("");
            let mut seg = self.int(get(7));
            if !get(8).is_empty() {
                let dm2 = self.int(get(8));
                if dm2 != 0 {
                    seg = (0x8000_0000u32 | seg.unsigned_abs() | (dm2.unsigned_abs() << 16)) as i32;
                }
            }
            let wire = Wire {
                start: Point3::new(self.num(get(0)), self.num(get(1)), self.num(get(2))),
                end: Point3::new(self.num(get(3)), self.num(get(4)), self.num(get(5))),
                radius: self.num(get(6)),
                segmentation: Segmentation::from_code(seg),
                note: String::new(),
            };
            self.project.antenna.wires.push(wire);
        }
        Some(())
    }

    fn read_sources(&mut self) -> Option<()> {
        let line = self.lines.data()?;
        let f = fields(&line);
        let count = self.uint(f[0]).max(0) as usize;
        self.project.antenna.auto_voltage = f.get(1).is_some_and(|s| self.uint(s) != 0);
        for _ in 0..count {
            let line = self.lines.data()?;
            let f = fields(&line);
            let get = |i: usize| f.get(i).copied().unwrap_or("");
            self.project.antenna.sources.push(Source {
                pulse: pulse_name(get(0)),
                phase_deg: self.num(get(1)),
                voltage: self.num(get(2)),
            });
        }
        Some(())
    }

    fn read_loads(&mut self) -> Option<()> {
        let line = self.lines.data()?;
        let f = fields(&line);
        let count = self.uint(f[0]).max(0) as usize;
        self.project.antenna.loads_enabled = f.get(1).is_some_and(|s| self.uint(s) != 0);
        for _ in 0..count {
            let line = self.lines.data()?;
            let f = fields(&line);
            let get = |i: usize| f.get(i).copied().unwrap_or("");
            let kind = match self.uint(get(1)) {
                0 => LoadKind::Lc {
                    inductance_uh: self.num(get(2)),
                    capacitance_pf: self.num(get(3)),
                    q: self.num(get(4)),
                },
                1 => LoadKind::Impedance { r: self.num(get(2)), x: self.num(get(3)) },
                2 => {
                    let mut coefficients = Vec::new();
                    let mut i = 2;
                    while coefficients.len() < crate::engine::SMAX && !get(i).is_empty() {
                        coefficients.push([self.num(get(i)), self.num(get(i + 1))]);
                        i += 2;
                    }
                    LoadKind::SDomain { coefficients }
                }
                t => {
                    self.warnings.push(format!("unknown load type {t}; treated as 0 Ω"));
                    LoadKind::Impedance { r: 0.0, x: 0.0 }
                }
            };
            self.project.antenna.loads.push(Load { pulse: pulse_name(get(0)), kind });
        }
        Some(())
    }

    fn read_segmentation_and_environment(&mut self) -> Option<()> {
        let Some(line) = self.lines.data() else {
            // The original leaves DM1 alone and resets the rest.
            let seg = &mut self.project.antenna.auto_segmentation;
            seg.dm2 = 40;
            seg.ec = 1;
            seg.sc = 2.0;
            return None;
        };
        let f = fields(&line);
        let get = |i: usize| f.get(i).copied().unwrap_or("");
        let mut seg = AutoSegmentation { dm1: self.uint(get(0)), dm2: self.uint(get(1)), sc: self.num(get(2)), ec: 1 };
        if seg.sc <= 1.0 || seg.sc > 3.0 {
            seg.sc = 2.0;
        }
        if !get(3).is_empty() {
            seg.ec = self.uint(get(3));
            if seg.ec <= 0 {
                seg.ec = 1;
            }
        }
        self.project.antenna.auto_segmentation = seg;

        let line = self.lines.data()?;
        let f = fields(&line);
        let get = |i: usize| f.get(i).copied().unwrap_or("");
        let env = &mut self.project.environment;
        let ground = eval_uint(get(0), &self.ctx).0;
        env.ground = if ground >= 3 { GroundKind::FreeSpace } else { GroundKind::from_code(ground) };
        env.height = eval(get(1), &self.ctx).0;
        if get(2).is_empty() {
            env.wire_material = WireMaterial::Lossless;
            return Some(());
        }
        env.wire_material = WireMaterial::from_code(eval_uint(get(2), &self.ctx).0);
        if !get(3).is_empty() {
            let (ro, ok) = eval(get(3), &self.ctx);
            if ok {
                env.swr_reference.r = ro;
                env.fb_range =
                    FbRange { horizontal: eval_uint(get(4), &self.ctx).0, vertical: eval_uint(get(5), &self.ctx).0 };
                if !get(6).is_empty() {
                    env.swr_reference.x = eval(get(6), &self.ctx).0;
                }
            }
        }
        Some(())
    }

    fn read_combined_wires(&mut self) -> Option<()> {
        let line = self.lines.data()?;
        let count = self.uint(&line).max(0) as usize;
        let mut defs = Vec::new();
        for _ in 0..count.min(crate::engine::PTMAX) {
            let line = self.lines.data()?;
            let f = fields(&line);
            let get = |i: usize| f.get(i).copied().unwrap_or("");
            let mut sections = Vec::new();
            let mut i = 2;
            while sections.len() < crate::engine::PPMAX && !get(i).is_empty() {
                sections.push(CombinedSection { length: self.num(get(i)), radius: self.num(get(i + 1)) });
                i += 2;
            }
            defs.push(CombinedWire { radius_key: self.num(get(0)), mode: self.uint(get(1)), sections });
        }
        self.project.antenna.combined_wires = defs;
        Some(())
    }
}

/// Pulse names are converted to half-width and copied into an 8-character
/// field.
fn pulse_name(s: &str) -> String {
    crate::engine::to_half_width(s).trim_end().chars().take(8).collect()
}

/// Moves the `[MMANA-WIRE-NOTES]` block from the comment onto the wires.
fn finish_comment(ant: &mut Antenna) {
    let mut clean = String::new();
    let mut in_block = false;
    for line in ant.comment.split("\r\n") {
        if line == WIRE_NOTE_BEGIN {
            in_block = true;
            continue;
        }
        if in_block {
            if line == WIRE_NOTE_END {
                in_block = false;
            } else if let Some((n, note)) = line.split_once('=') {
                let w: usize = n.trim().parse().unwrap_or(0);
                if w > 0 && w <= ant.wires.len() {
                    ant.wires[w - 1].note = decode_note(note);
                }
            }
            continue;
        }
        clean.push_str(line);
        clean.push_str("\r\n");
    }
    // split() yields a trailing empty piece after the final CRLF.
    if clean.ends_with("\r\n\r\n") || clean == "\r\n" {
        clean.truncate(clean.len() - 2);
    }
    ant.comment = clean;
}

/// Notes are stored percent-encoded (bytes in the file's encoding).
fn decode_note(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2]))
        {
            out.push(h << 4 | l);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    decode(&out)
}

fn hex(c: u8) -> Option<u8> {
    (c as char).to_digit(16).map(|d| d as u8)
}

fn encode_note(s: &str) -> String {
    let mut out = String::new();
    for &c in encode(s).iter() {
        if c.is_ascii_alphanumeric() || b" ._-/:()[]".contains(&c) {
            out.push(c as char);
        } else {
            out.push_str(&format!("%{c:02X}"));
        }
    }
    out
}

/// Serialises a project in MMANA's `.maa` layout (CRLF line endings). Use
/// [`encode`] to obtain the Shift_JIS bytes MMANA expects.
pub fn write(project: &Project) -> String {
    let ant = &project.antenna;
    let env = &project.environment;
    let mut out = String::new();
    let mut line = |s: String| {
        out.push_str(&s);
        out.push_str("\r\n");
    };
    line(ant.name.clone());
    line("*".into());
    line(str_dbl(ant.frequency_mhz));
    line("*** ﾜｲﾔ ***".into());
    if ant.stack.is_stacked() {
        let dir = match ant.stack.direction {
            StackDirection::Centered => 0,
            StackDirection::Up => 1,
            StackDirection::Down => 2,
        };
        line(format!(
            "{},\t{},\t{},\t{},\t{},\t{dir}",
            ant.wires.len(),
            ant.stack.horizontal,
            ant.stack.vertical,
            str_dbl_e(ant.stack.horizontal_spacing),
            str_dbl_e(ant.stack.vertical_spacing)
        ));
    } else {
        line(ant.wires.len().to_string());
    }
    for w in &ant.wires {
        let seg = match w.segmentation {
            Segmentation::TaperCustom { dm1, dm2: Some(dm2) } => format!("{dm1},{dm2}"),
            s => s.to_code().to_string(),
        };
        line(format!(
            "{},\t{},\t{},\t{},\t{},\t{},\t{},\t{seg}",
            str_dbl_e(w.start.x),
            str_dbl_e(w.start.y),
            str_dbl_e(w.start.z),
            str_dbl_e(w.end.x),
            str_dbl_e(w.end.y),
            str_dbl_e(w.end.z),
            str_dbl_e(w.radius)
        ));
    }
    line("*** 給電点 ***".into());
    line(format!("{},\t{}", ant.sources.len(), ant.auto_voltage as i32));
    for s in &ant.sources {
        line(format!("{},\t{},\t{}", s.pulse, str_dbl_e(s.phase_deg), str_dbl_e(s.voltage)));
    }
    line("*** 集中定数 ***".into());
    line(format!("{},\t{}", ant.loads.len(), ant.loads_enabled as i32));
    for l in &ant.loads {
        let body = match &l.kind {
            LoadKind::Lc { inductance_uh, capacitance_pf, q } => {
                format!("0,\t{},\t{},\t{}", str_dbl_e(*inductance_uh), str_dbl_e(*capacitance_pf), str_dbl_e(*q))
            }
            LoadKind::Impedance { r, x } => format!("1,\t{},\t{}", str_dbl_e(*r), str_dbl_e(*x)),
            LoadKind::SDomain { coefficients } => {
                let pairs: Vec<String> =
                    coefficients.iter().map(|[a, b]| format!("{},\t{}", str_dbl_e(*a), str_dbl_e(*b))).collect();
                format!("2,\t{}", pairs.join(",\t"))
            }
        };
        line(format!("{},\t{body}", l.pulse));
    }
    let seg = &ant.auto_segmentation;
    line("*** 自動分割 ***".into());
    line(format!("{},\t{},\t{},\t{}", seg.dm1, seg.dm2, str_dbl(seg.sc), seg.ec));
    line("*** 計算環境 ***".into());
    // MMANA writes jX with "%.lf" (no decimals); the full value is kept here.
    line(format!(
        "{},\t{},\t{},\t{},\t{},\t{},\t{}",
        env.ground.code(),
        str_dbl_e(env.height),
        env.wire_material.code(),
        str_dbl_e(env.swr_reference.r),
        env.fb_range.horizontal,
        env.fb_range.vertical,
        str_dbl_e(env.swr_reference.x)
    ));
    if !ant.combined_wires.is_empty() {
        line("$$$ ワイヤ組み合わせデータ $$$".into());
        line(ant.combined_wires.len().to_string());
        for c in &ant.combined_wires {
            let pairs: Vec<String> = c
                .sections
                .iter()
                .take_while(|s| s.length != 0.0)
                .map(|s| format!("{},\t{}", str_dbl_e(s.length), str_dbl_e(s.radius)))
                .collect();
            line(format!("{},\t{},\t{}", str_dbl_e(c.radius_key), c.mode, pairs.join(",\t")));
        }
    }
    let mut rem = ant.comment.clone();
    let notes: Vec<String> = ant
        .wires
        .iter()
        .enumerate()
        .filter(|(_, w)| !w.note.is_empty())
        .map(|(i, w)| format!("{}={}", i + 1, encode_note(&w.note)))
        .collect();
    if !notes.is_empty() {
        if !rem.is_empty() && !rem.ends_with('\n') {
            rem.push_str("\r\n");
        }
        rem.push_str(WIRE_NOTE_BEGIN);
        rem.push_str("\r\n");
        for n in notes {
            rem.push_str(&n);
            rem.push_str("\r\n");
        }
        rem.push_str(WIRE_NOTE_END);
        rem.push_str("\r\n");
    }
    if !rem.is_empty() {
        out.push_str("### コメント ###\r\n");
        out.push_str(&rem);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const DP: &str = "Dipole\r\n*\r\n14.05\r\n*** ﾜｲﾔ ***\r\n1\r\n0.0,\t-5.0,\t0.0,\t0.0,\t5.0,\t0.0,\t8.000e-04,\t-1\r\n*** 給電点 ***\r\n1,\t1\r\nw1c,\t0.0,\t1.0\r\n*** 集中定数 ***\r\n0,\t1\r\n*** 自動分割 ***\r\n400,\t40,\t2.0,\t1\r\n*** 計算環境 ***\r\n2,\t20.0,\t0,\t50.0,\t120,\t60,\t0.0\r\n";

    #[test]
    fn parses_basic_file() {
        let p = parse(DP);
        assert!(p.warnings.is_empty(), "{:?}", p.warnings);
        let a = &p.project.antenna;
        assert_eq!(a.name, "Dipole");
        assert_eq!(a.frequency_mhz, 14.05);
        assert_eq!(a.wires.len(), 1);
        assert_eq!(a.wires[0].radius, 8.0e-4);
        assert_eq!(a.wires[0].segmentation, Segmentation::Taper);
        assert_eq!(a.sources[0].pulse, "w1c");
        assert_eq!(p.project.environment.ground, GroundKind::Real);
    }

    #[test]
    fn round_trips() {
        let p = parse(DP).project;
        assert_eq!(write(&p), DP);
    }

    #[test]
    fn full_width_pulse_names_are_converted() {
        let p = parse(&DP.replace("w1c,", "ｗ１ｃ,")).project;
        assert_eq!(p.antenna.sources[0].pulse, "w1c");
    }

    #[test]
    fn wire_notes_round_trip() {
        let mut p = parse(DP).project;
        p.antenna.wires[0].note = "給電 element".into();
        p.antenna.comment = "hello\r\n".into();
        let text = write(&p);
        let back = parse(&decode(&encode(&text))).project;
        assert_eq!(back.antenna.wires[0].note, "給電 element");
        assert_eq!(back.antenna.comment, "hello\r\n");
    }
}
