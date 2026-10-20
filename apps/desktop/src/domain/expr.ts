// Numeric fields accept simple expressions like MMANA: `R/4`, `0.5*I`, `(1+2)/3`.
// R = wavelength at the design frequency, I = inch, F = foot.
// (A conventional precedence parser; the engine's quirk-compatible version is
// only needed for reading .maa files and lives in mmana-core.)

export interface ExprContext {
  wavelength: number;
}

const FULLWIDTH = /[！-～]/g;

export function evaluate(text: string, ctx: ExprContext): number | null {
  const src = text.replace(FULLWIDTH, (c) => String.fromCharCode(c.charCodeAt(0) - 0xfee0)).replace(/\s+/g, '');
  if (src === '') return null;
  let pos = 0;
  const peek = () => src[pos];

  const number = (): number | null => {
    const m = /^(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?/.exec(src.slice(pos));
    if (m) {
      pos += m[0].length;
      return Number(m[0]);
    }
    const unit = src[pos]?.toUpperCase();
    const units: Record<string, number> = { R: ctx.wavelength, I: 0.0254, F: 0.3048 };
    if (unit && unit in units) {
      pos += 1;
      return units[unit];
    }
    return null;
  };
  const factor = (): number | null => {
    if (peek() === '-') {
      pos += 1;
      const v = factor();
      return v == null ? null : -v;
    }
    if (peek() === '+') {
      pos += 1;
      return factor();
    }
    if (peek() === '(') {
      pos += 1;
      const v = sum();
      if (peek() !== ')') return null;
      pos += 1;
      return v;
    }
    return number();
  };
  const product = (): number | null => {
    let v = factor();
    while (v != null && (peek() === '*' || peek() === '/')) {
      const op = src[pos++];
      const r = factor();
      if (r == null) return null;
      v = op === '*' ? v * r : v / r;
    }
    return v;
  };
  const sum = (): number | null => {
    let v = product();
    while (v != null && (peek() === '+' || peek() === '-')) {
      const op = src[pos++];
      const r = product();
      if (r == null) return null;
      v = op === '+' ? v + r : v - r;
    }
    return v;
  };

  const v = sum();
  return v != null && pos === src.length && Number.isFinite(v) ? v : null;
}
