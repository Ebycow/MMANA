// ツール → 計算ツール…: the calculators of MMANA's 共振条件 dialog. Each edit
// (committed on blur or Enter) asks the engine for the values the original's
// edit handler recalculates; the fields show what comes back.
import { Alert, Button, Checkbox, Group, Modal, Radio, Select, SimpleGrid, Stack, Table, Tabs, Text, TextInput } from '@mantine/core';
import { IconAlertTriangle } from '@tabler/icons-react';
import { useEffect, useRef, useState, type ReactNode } from 'react';
import { transport } from '../api/transport';
import type { CircuitInput, CircuitRequest, CircuitResponse, CircuitResult, Complex, Lumped } from '../api/types';
import {
  CABLES,
  DEFAULT_STUB_CABLE,
  sig,
  stubGiven,
  tankInput,
  type StubField,
  type TankField,
  type TankValues,
} from '../domain/circuits';
import { evaluate } from '../domain/expr';
import { useCircuits, type CircuitTab } from '../state/circuits';
import { useApp } from '../state/store';

type Tool = CircuitRequest['tool'];

/** What the dialog starts from: the last calculation, or the design frequency and 50 Ω. */
interface Seed {
  frequencyMhz: number;
  impedance: Complex;
  /** The SWR reference R. */
  lineOhm: number;
}

function useSeed(): Seed {
  const result = useApp((s) => s.result);
  const project = useApp((s) => s.project);
  return {
    frequencyMhz: result?.frequencyMhz ?? project.antenna.frequencyMhz,
    impedance: result?.impedance ?? { re: 50, im: 0 },
    lineOhm: project.environment.swrReference.r,
  };
}

/**
 * Runs calculations one after another; a result that arrives after a later
 * request was sent is dropped. Errors are shown under the page.
 */
function useCalc() {
  const seq = useRef(0);
  const [error, setError] = useState<string | null>(null);
  async function call(req: CircuitRequest): Promise<CircuitResponse['result'] | null> {
    const n = ++seq.current;
    try {
      const r = await transport.circuit(req);
      if (n !== seq.current) return null;
      setError(null);
      return r.result;
    } catch (e) {
      if (n === seq.current) setError((e as { detail?: string }).detail ?? String(e));
      return null;
    }
  }
  const run = <T extends Tool>(tool: T, input: CircuitInput<T>) => call({ tool, input } as CircuitRequest) as Promise<CircuitResult<T> | null>;
  return { run, error };
}

const positive = (v: number) => v > 0;
const nonNegative = (v: number) => v >= 0;

/** A number field that takes expressions (`1/(2*3.14)`), committed on blur or Enter. */
function ValueField({
  label,
  unit,
  value,
  onCommit,
  readOnly,
  validate,
  digits,
}: {
  label: string;
  unit?: string;
  value: number | null | undefined;
  onCommit?: (v: number) => void;
  readOnly?: boolean;
  validate?: (v: number) => boolean;
  digits?: number;
}) {
  const shown = sig(value, digits);
  const [text, setText] = useState(shown);
  const [bad, setBad] = useState(false);
  useEffect(() => {
    setText(shown);
    setBad(false);
  }, [shown]);
  const commit = () => {
    if (readOnly || text === shown) return setBad(false);
    // R (wavelength) means nothing here
    const v = evaluate(text, { wavelength: Number.NaN });
    if (v == null || !Number.isFinite(v) || (validate && !validate(v))) return setBad(true);
    setBad(false);
    onCommit?.(v);
  };
  return (
    <TextInput
      size="xs"
      label={unit ? `${label} (${unit})` : label}
      value={text}
      readOnly={readOnly}
      variant={readOnly ? 'filled' : 'default'}
      error={bad}
      classNames={{ input: 'num' }}
      onChange={(e) => setText(e.currentTarget.value)}
      onBlur={commit}
      onKeyDown={(e) => e.key === 'Enter' && commit()}
    />
  );
}

function ImpedanceFields({ label, value, onCommit, readOnly }: { label: string; value: Complex | null | undefined; onCommit?: (z: Complex) => void; readOnly?: boolean }) {
  return (
    <SimpleGrid cols={2} spacing="xs">
      <ValueField label={`${label} R`} unit="Ω" value={value?.re} readOnly={readOnly} validate={nonNegative} onCommit={(re) => onCommit?.({ re, im: value?.im ?? 0 })} />
      <ValueField label={`${label} jX`} unit="Ω" value={value?.im} readOnly={readOnly} onCommit={(im) => onCommit?.({ re: value?.re ?? 0, im })} />
    </SimpleGrid>
  );
}

function elementText(e: Lumped | null | undefined): string {
  if (!e) return '';
  return e.kind === 'L' ? `L ${sig(e.value, 4)} µH` : `C ${sig(e.value, 4)} pF`;
}

function Note({ children }: { children: ReactNode }) {
  return (
    <Text size="xs" c="dimmed">
      {children}
    </Text>
  );
}

function ErrorLine({ error }: { error: string | null }) {
  return error ? (
    <Text size="xs" c="red">
      {error}
    </Text>
  ) : null;
}

function Warning({ children }: { children: ReactNode }) {
  return (
    <Alert variant="light" color="yellow" p="xs" icon={<IconAlertTriangle size={16} />}>
      <Text size="xs">{children}</Text>
    </Alert>
  );
}

// ---------------------------------------------------------- LC resonance ---

function TankPage({ seed }: { seed: Seed }) {
  const { run, error } = useCalc();
  const [v, setV] = useState<TankValues>({
    frequencyMhz: seed.frequencyMhz,
    reactanceOhm: Math.abs(seed.impedance.im) || null,
    inductanceUh: null,
    capacitancePf: null,
    fixedFrequency: true,
    last: 'reactance',
  });
  const [wl, setWl] = useState<CircuitResult<'lcResonance'>['wavelength']>(null);
  const calc = async (next: TankValues, field: TankField) => {
    const input = tankInput(next, field);
    setV(next);
    if (!input) return;
    const r = await run('lcResonance', input);
    if (!r) return;
    setV({ ...next, frequencyMhz: r.frequencyMhz ?? null, reactanceOhm: r.reactanceOhm ?? null, inductanceUh: r.inductanceUh ?? null, capacitancePf: r.capacitancePf ?? null });
    setWl(r.wavelength ?? null);
  };
  useEffect(() => void calc(v, 'reactance'), []);
  const edit = (field: Exclude<TankField, 'frequency'>, key: 'reactanceOhm' | 'inductanceUh' | 'capacitancePf') => (x: number) =>
    calc({ ...v, [key]: x, last: field }, field);
  return (
    <Stack gap="sm">
      <Note>周波数と、X・L・C のどれか 1 つから残りを求めます。L と C は、その周波数で同じ大きさのリアクタンスを持つもの（共振する組）です。</Note>
      <SimpleGrid cols={2} spacing="xs">
        <ValueField label="周波数" unit="MHz" value={v.frequencyMhz} validate={positive} onCommit={(f) => calc({ ...v, frequencyMhz: f }, 'frequency')} />
        <ValueField label="X" unit="Ω" value={v.reactanceOhm} onCommit={edit('reactance', 'reactanceOhm')} />
        <ValueField label="L" unit="µH" value={v.inductanceUh} validate={positive} onCommit={edit('inductance', 'inductanceUh')} />
        <ValueField label="C" unit="pF" value={v.capacitancePf} validate={positive} onCommit={edit('capacitance', 'capacitancePf')} />
      </SimpleGrid>
      <Checkbox
        size="xs"
        label="周波数を固定（外すと、L か C を変えたとき、もう一方を保って共振周波数を求めます）"
        checked={v.fixedFrequency}
        onChange={(e) => setV({ ...v, fixedFrequency: e.currentTarget.checked })}
      />
      {wl && (
        <Table withTableBorder fz="xs" w={320}>
          <Table.Tbody>
            {(
              [
                ['λ', wl.fullM],
                ['λ/2', wl.halfM],
                ['λ/4', wl.quarterM],
                ['5/8 λ', wl.fiveEighthsM],
              ] as const
            ).map(([k, m]) => (
              <Table.Tr key={k}>
                <Table.Td>{k}</Table.Td>
                <Table.Td ta="right" className="num">
                  {sig(m, 5)} m
                </Table.Td>
              </Table.Tr>
            ))}
          </Table.Tbody>
        </Table>
      )}
      <Note>波長は自由空間で、λ = 299.8 / f（MHz）です。</Note>
      <ErrorLine error={error} />
    </Stack>
  );
}

// ------------------------------------------------------------------ coil ---

type CoilSolve = 'inductance' | 'turns' | 'diameter';

function CoilPage() {
  const { run, error } = useCalc();
  const [solve, setSolve] = useState<CoilSolve>('inductance');
  // 10 turns of 1 mm wire, 1 mm apart, as the original; 2 cm across instead
  // of its 10 cm, which is too short a coil for the formula
  const [v, setV] = useState({ inductanceUh: 0, turns: 10, diameterCm: 2, wireDiameterMm: 1, spacingMm: 1 });
  const [r, setR] = useState<CircuitResult<'coil'> | null>(null);
  const calc = async (next: typeof v, s: CoilSolve) => {
    setV(next);
    const unknown =
      s === 'inductance'
        ? { solveFor: s, turns: next.turns, diameterCm: next.diameterCm }
        : s === 'turns'
          ? { solveFor: s, inductanceUh: next.inductanceUh, diameterCm: next.diameterCm }
          : { solveFor: s, inductanceUh: next.inductanceUh, turns: next.turns };
    if (s !== 'inductance' && !(next.inductanceUh > 0)) return setR(null);
    const res = await run('coil', { ...unknown, wireDiameterMm: next.wireDiameterMm, spacingMm: next.spacingMm } as CircuitInput<'coil'>);
    if (!res) return;
    setR(res);
    setV({ ...next, inductanceUh: res.inductanceUh, turns: res.turns, diameterCm: res.diameterCm });
  };
  useEffect(() => void calc(v, solve), []);
  const field = (key: keyof typeof v) => (x: number) => calc({ ...v, [key]: x }, solve);
  return (
    <Stack gap="sm">
      <Note>1 層巻きの空芯コイルです。Wheeler の式（L = D²N² / (45 D + 102 ℓ)、D・ℓ は cm、L は µH）で求めます。</Note>
      <Radio.Group
        size="xs"
        label="求める値"
        value={solve}
        onChange={(s) => {
          setSolve(s as CoilSolve);
          calc(v, s as CoilSolve);
        }}
      >
        <Group gap="md" mt={4}>
          <Radio value="inductance" label="L" />
          <Radio value="turns" label="巻数" />
          <Radio value="diameter" label="直径" />
        </Group>
      </Radio.Group>
      <SimpleGrid cols={2} spacing="xs">
        <ValueField label="L" unit="µH" value={v.inductanceUh || null} readOnly={solve === 'inductance'} validate={positive} onCommit={field('inductanceUh')} />
        <ValueField label="巻数" unit="回" value={v.turns} readOnly={solve === 'turns'} validate={positive} onCommit={field('turns')} />
        <ValueField label="コイルの直径" unit="cm" value={v.diameterCm} readOnly={solve === 'diameter'} validate={positive} onCommit={field('diameterCm')} />
        <ValueField label="線の直径" unit="mm" value={v.wireDiameterMm} validate={positive} onCommit={field('wireDiameterMm')} />
        <ValueField label="巻きスペース（線と線のすき間）" unit="mm" value={v.spacingMm} validate={nonNegative} onCommit={field('spacingMm')} />
        <ValueField label="コイルの長さ" unit="cm" value={r?.lengthCm} readOnly />
      </SimpleGrid>
      <Note>直径は巻いた線の中心どうしの径、長さは（線の直径 + 巻きスペース）× 巻数です。</Note>
      {r?.short && <Warning>コイルの長さが直径の 0.4 倍より短いため、この式の誤差は 1 % を超えます。</Warning>}
      <ErrorLine error={error} />
    </Stack>
  );
}

// -------------------------------------------------------------- LC match ---

/** The L network as drawn by the original (PBoxMPaint): the line on the left, the antenna on the right. */
function MatchDiagram({ m, lineOhm, antenna }: { m: CircuitResult<'lcMatch'>; lineOhm: number; antenna: Complex }) {
  const W = 420;
  const top = 40;
  const bottom = 130;
  const shuntX = m.shuntSide === 'line' ? 110 : 310;
  const seriesX = m.shuntSide === 'line' ? 220 : 200;
  const label = (e: Lumped) => `${sig(e.reactanceOhm, 4)} Ω`;
  return (
    <svg viewBox={`0 0 ${W} 160`} width="100%" style={{ maxWidth: W }} role="img" aria-label="整合回路">
      <g stroke="currentColor" strokeWidth={1.5} fill="none">
        <line x1={20} y1={top} x2={m.series ? seriesX - 28 : W - 20} y2={top} />
        {m.series && <rect x={seriesX - 28} y={top - 9} width={56} height={18} rx={3} />}
        {m.series && <line x1={seriesX + 28} y1={top} x2={W - 20} y2={top} />}
        <line x1={20} y1={bottom} x2={W - 20} y2={bottom} />
        {m.shunt && (
          <>
            <line x1={shuntX} y1={top} x2={shuntX} y2={(top + bottom) / 2 - 18} />
            <rect x={shuntX - 9} y={(top + bottom) / 2 - 18} width={18} height={36} rx={3} />
            <line x1={shuntX} y1={(top + bottom) / 2 + 18} x2={shuntX} y2={bottom} />
          </>
        )}
      </g>
      <g fill="currentColor" fontSize={11}>
        <text x={20} y={top - 22}>
          給電線 {sig(lineOhm, 4)} Ω
        </text>
        <text x={W - 20} y={top - 22} textAnchor="end">
          アンテナ {sig(antenna.re, 4)} {antenna.im < 0 ? '−' : '+'} j{sig(Math.abs(antenna.im), 4)} Ω
        </text>
        {m.series && (
          <>
            <text x={seriesX} y={top - 14} textAnchor="middle">
              {elementText(m.series)}
            </text>
            <text x={seriesX} y={top + 24} textAnchor="middle">
              {label(m.series)}
            </text>
          </>
        )}
        {m.shunt && (
          <>
            <text x={shuntX + 16} y={(top + bottom) / 2 - 2}>
              {elementText(m.shunt)}
            </text>
            <text x={shuntX + 16} y={(top + bottom) / 2 + 12}>
              {label(m.shunt)}
            </text>
          </>
        )}
      </g>
    </svg>
  );
}

function MatchPage({ seed }: { seed: Seed }) {
  const { run, error } = useCalc();
  const [input, setInput] = useState<CircuitInput<'lcMatch'>>({ frequencyMhz: seed.frequencyMhz, lineOhm: seed.lineOhm, antenna: seed.impedance, circuit: 'shuntC' });
  const [m, setM] = useState<CircuitResult<'lcMatch'> | null>(null);
  const calc = async (next: CircuitInput<'lcMatch'>) => {
    setInput(next);
    if (!(next.antenna.re > 0)) return setM(null);
    const r = await run('lcMatch', next);
    if (r) setM(r);
  };
  useEffect(() => void calc(input), []);
  return (
    <Stack gap="sm">
      <Note>L と C の 2 素子（L 形回路）で、アンテナを給電線に整合します。最初の値は直前の計算結果と SWR 基準の R です。</Note>
      <SimpleGrid cols={2} spacing="xs">
        <ValueField label="周波数" unit="MHz" value={input.frequencyMhz} validate={positive} onCommit={(f) => calc({ ...input, frequencyMhz: f })} />
        <ValueField label="給電線の Z" unit="Ω" value={input.lineOhm} validate={positive} onCommit={(z) => calc({ ...input, lineOhm: z })} />
      </SimpleGrid>
      <ImpedanceFields label="アンテナ" value={input.antenna} onCommit={(antenna) => calc({ ...input, antenna })} />
      <Radio.Group size="xs" label="回路" value={input.circuit ?? 'shuntC'} onChange={(c) => calc({ ...input, circuit: c as 'shuntC' | 'seriesC' })}>
        <Group gap="md" mt={4}>
          <Radio value="shuntC" label="C 並列型" />
          <Radio value="seriesC" label="C 直列型" />
        </Group>
      </Radio.Group>
      {m && !m.shunt && !m.series && <Text size="sm">整合はいりません。</Text>}
      {m && (m.shunt || m.series) && <MatchDiagram m={m} lineOhm={input.lineOhm} antenna={input.antenna} />}
      <Note>
        並列の素子は、アンテナの R が給電線の Z 以下なら給電線の側、上回ればアンテナの側に入ります。素子の損失と浮遊容量は含みません。
      </Note>
      <ErrorLine error={error} />
    </Stack>
  );
}

// ------------------------------------------------------------------ stub ---

const CUSTOM = '任意';

function CableFields({ cable, name, onChange }: { cable: { impedanceOhm: number; velocityFactor: number }; name: string; onChange: (name: string, cable: { impedanceOhm: number; velocityFactor: number }) => void }) {
  const custom = name === CUSTOM;
  return (
    <SimpleGrid cols={3} spacing="xs">
      <Select
        size="xs"
        label="ケーブル"
        data={[CUSTOM, ...CABLES.map((c) => c.name)]}
        value={name}
        allowDeselect={false}
        onChange={(n) => {
          const preset = CABLES.find((c) => c.name === n);
          onChange(n ?? CUSTOM, preset ? { impedanceOhm: preset.impedanceOhm, velocityFactor: preset.velocityFactor } : cable);
        }}
      />
      <ValueField label="Z₀" unit="Ω" value={cable.impedanceOhm} readOnly={!custom} validate={positive} onCommit={(z) => onChange(name, { ...cable, impedanceOhm: z })} />
      <ValueField label="短縮率" value={cable.velocityFactor} digits={4} readOnly={!custom} validate={positive} onCommit={(p) => onChange(name, { ...cable, velocityFactor: p })} />
    </SimpleGrid>
  );
}

function StubPage({ seed }: { seed: Seed }) {
  const { run, error } = useCalc();
  const preset = CABLES.find((c) => c.name === DEFAULT_STUB_CABLE)!;
  const [frequencyMhz, setFrequency] = useState(seed.frequencyMhz);
  const [cableName, setCableName] = useState(preset.name);
  const [cable, setCable] = useState({ impedanceOhm: preset.impedanceOhm, velocityFactor: preset.velocityFactor });
  const [end, setEnd] = useState<'open' | 'short'>('open');
  const [v, setV] = useState({ reactanceOhm: -50 as number | null, value: 0, inductive: false, lengthCm: 0 });
  const [fo, setFo] = useState<number | null>(null);
  const calc = async (field: StubField, next = v, o = { frequencyMhz, cable, end }) => {
    setV(next);
    const r = await run('stub', { ...o, given: stubGiven(next, field) });
    if (!r) return;
    setV({ reactanceOhm: r.reactanceOhm ?? null, value: r.element.value, inductive: r.element.kind === 'L', lengthCm: r.lengthCm });
    setFo(r.quarterWaveMhz ?? null);
  };
  useEffect(() => void calc('reactance'), []);
  return (
    <Stack gap="sm">
      <Note>ケーブルの先を開放またはショートしたスタブを、リアクタンス（L・C の代わり）として使うときの長さです。リアクタンス・L または C・長さのどれかから残りを求めます。</Note>
      <ValueField
        label="周波数"
        unit="MHz"
        value={frequencyMhz}
        validate={positive}
        onCommit={(f) => {
          setFrequency(f);
          calc('other', v, { frequencyMhz: f, cable, end });
        }}
      />
      <CableFields
        cable={cable}
        name={cableName}
        onChange={(n, c) => {
          setCableName(n);
          setCable(c);
          calc('other', v, { frequencyMhz, cable: c, end });
        }}
      />
      <Radio.Group
        size="xs"
        label="先端"
        value={end}
        onChange={(e) => {
          setEnd(e as 'open' | 'short');
          calc('end', v, { frequencyMhz, cable, end: e as 'open' | 'short' });
        }}
      >
        <Group gap="md" mt={4}>
          <Radio value="open" label="開放" />
          <Radio value="short" label="ショート" />
        </Group>
      </Radio.Group>
      <SimpleGrid cols={3} spacing="xs">
        <ValueField label="リアクタンス" unit="Ω" value={v.reactanceOhm} onCommit={(x) => calc('reactance', { ...v, reactanceOhm: x })} />
        <ValueField label={v.inductive ? 'L' : 'C'} unit={v.inductive ? 'µH' : 'pF'} value={v.value} validate={nonNegative} onCommit={(x) => calc('value', { ...v, value: x })} />
        <ValueField label="長さ" unit="cm" value={v.lengthCm} validate={nonNegative} onCommit={(x) => calc('length', { ...v, lengthCm: x })} />
      </SimpleGrid>
      {v.reactanceOhm == null && <Text size="xs">リアクタンスは ∞（開放）です。</Text>}
      {fo != null && <Text size="xs">この長さが 1/4 波長になる周波数: {sig(fo, 5)} MHz</Text>}
      <Note>長さは半波長より短いものです。L か C かはリアクタンスの符号で決まり、正なら L、負なら C です。ケーブルの損失は含みません。</Note>
      <ErrorLine error={error} />
    </Stack>
  );
}

// ------------------------------------------------------ transmission line ---

function LinePage({ seed }: { seed: Seed }) {
  const { run, error } = useCalc();
  const [line, setLine] = useState({ impedanceOhm: seed.lineOhm, lengthWl: 0.25, lossDb: 0 });
  const [toLoad, setToLoad] = useState(false);
  const [input, setInputZ] = useState<Complex | null>(null);
  const [load, setLoad] = useState<Complex | null>(seed.impedance);
  const [swr, setSwr] = useState<{ input: number | null; load: number | null }>({ input: null, load: null });
  const calc = async (known: CircuitInput<'line'>['known'], l = line) => {
    setLine(l);
    if (known.from === 'load') setLoad(known.impedance);
    else setInputZ(known.impedance);
    const r = await run('line', { ...l, known });
    if (!r) return;
    setInputZ(r.input ?? null);
    setLoad(r.load ?? null);
    setSwr({ input: r.inputSwr ?? null, load: r.loadSwr ?? null });
  };
  // a change of the line keeps the end ZL を計算 does not change
  const fromKept = (l: typeof line) => {
    const z = toLoad ? input : load;
    if (z) calc({ from: toLoad ? 'input' : 'load', impedance: z }, l);
  };
  useEffect(() => void calc({ from: 'load', impedance: seed.impedance }), []);
  return (
    <Stack gap="sm">
      <Note>長さ（波長）の給電線の一方の端のインピーダンスから、もう一方の端のインピーダンスを求めます。損失は線路全体の値（dB）です。</Note>
      <SimpleGrid cols={3} spacing="xs">
        <ValueField label="線路の Z₀" unit="Ω" value={line.impedanceOhm} validate={positive} onCommit={(z) => fromKept({ ...line, impedanceOhm: z })} />
        <ValueField label="長さ" unit="λ" value={line.lengthWl} validate={nonNegative} onCommit={(x) => fromKept({ ...line, lengthWl: x })} />
        <ValueField label="損失" unit="dB" value={line.lossDb} validate={nonNegative} onCommit={(x) => fromKept({ ...line, lossDb: x })} />
      </SimpleGrid>
      <ImpedanceFields label="入力 Zi" value={input} onCommit={(z) => calc({ from: 'input', impedance: z })} />
      <ImpedanceFields label="負荷 ZL" value={load} onCommit={(z) => calc({ from: 'load', impedance: z })} />
      <Checkbox
        size="xs"
        label="ZL を計算（Z₀・長さ・損失を変えたとき、Zi を保って ZL を求めます）"
        checked={toLoad}
        onChange={(e) => setToLoad(e.currentTarget.checked)}
      />
      <Text size="xs" className="num">
        SWR（Z₀ に対して）入力 {sig(swr.input, 4) || '—'}、負荷 {sig(swr.load, 4) || '—'}
      </Text>
      <Note>長さは電気長です。物理長は、波長 × 短縮率で求めてください。</Note>
      <ErrorLine error={error} />
    </Stack>
  );
}

// ------------------------------------------------------ two-section match ---

function TwoSectionPage({ seed }: { seed: Seed }) {
  const { run, error } = useCalc();
  // the original's defaults: 75 Ω then 50 Ω towards the load
  const [input, setInput] = useState<CircuitInput<'twoSection'>>({
    load: seed.impedance,
    referenceOhm: seed.lineOhm,
    feedSection: { impedanceOhm: 75, lengthWl: 0 },
    loadSection: { impedanceOhm: 50, lengthWl: 0 },
  });
  const [r, setR] = useState<CircuitResult<'twoSection'> | null>(null);
  const calc = async (next: CircuitInput<'twoSection'>) => {
    setInput(next);
    const res = await run('twoSection', next);
    if (res) setR(res);
  };
  const tune = async () => {
    const res = await run('twoSectionTune', {
      load: input.load,
      referenceOhm: input.referenceOhm,
      feedImpedanceOhm: input.feedSection.impedanceOhm,
      loadImpedanceOhm: input.loadSection.impedanceOhm,
    });
    if (!res) return;
    setR(res);
    setInput({ ...input, feedSection: res.feedSection, loadSection: res.loadSection });
  };
  useEffect(() => void calc(input), []);
  const section = (key: 'feedSection' | 'loadSection', label: string) => (
    <SimpleGrid cols={2} spacing="xs">
      <ValueField label={`${label} Z₀`} unit="Ω" value={input[key].impedanceOhm} validate={positive} onCommit={(z) => calc({ ...input, [key]: { ...input[key], impedanceOhm: z } })} />
      <ValueField label={`${label} 長さ`} unit="λ" value={input[key].lengthWl} validate={nonNegative} onCommit={(l) => calc({ ...input, [key]: { ...input[key], lengthWl: l } })} />
    </SimpleGrid>
  );
  return (
    <Stack gap="sm">
      <Note>
        インピーダンスの違う 2 本の線路を直列にして整合します（Q マッチ、シリーズセクション）。給電線 ― 線路 1 ― 線路 2 ― 負荷の順です。「長さを求める」で、SWR が最も下がる長さを探します。
      </Note>
      <SimpleGrid cols={3} spacing="xs">
        <ValueField label="負荷 R" unit="Ω" value={input.load.re} validate={nonNegative} onCommit={(re) => calc({ ...input, load: { ...input.load, re } })} />
        <ValueField label="負荷 jX" unit="Ω" value={input.load.im} onCommit={(im) => calc({ ...input, load: { ...input.load, im } })} />
        <ValueField label="整合先の R" unit="Ω" value={input.referenceOhm} validate={positive} onCommit={(referenceOhm) => calc({ ...input, referenceOhm })} />
      </SimpleGrid>
      {section('feedSection', '線路 1（給電線の側）')}
      {section('loadSection', '線路 2（負荷の側）')}
      <Group>
        <Button size="xs" onClick={tune}>
          長さを求める
        </Button>
        {r && (
          <Text size="xs" className="num">
            入力 {sig(r.input?.re, 5)} {(r.input?.im ?? 0) < 0 ? '−' : '+'} j{sig(Math.abs(r.input?.im ?? 0), 4)} Ω、SWR {sig(r.swr, 4) || '—'}（{sig(input.referenceOhm, 4)} Ω に対して）
          </Text>
        )}
      </Group>
      <Note>長さは電気長（波長）で、0.0001 λ 刻みで探します。損失は含みません。</Note>
      <ErrorLine error={error} />
    </Stack>
  );
}

// ------------------------------------------------------------ stub match ---

/**
 * The SWR a stub match stays above only when the line cannot match: the
 * 0.0001 λ steps leave about 1.01 at a true match.
 */
const STUB_MATCHED_SWR = 1.05;

function StubMatchPage({ seed }: { seed: Seed }) {
  const { run, error } = useCalc();
  // the original's defaults: a 600 Ω line of velocity factor 0.95
  const [input, setInput] = useState<CircuitInput<'stubMatch'>>({
    frequencyMhz: seed.frequencyMhz,
    load: seed.impedance,
    referenceOhm: seed.lineOhm,
    cable: { impedanceOhm: 600, velocityFactor: 0.95 },
  });
  const [cableName, setCableName] = useState(CUSTOM);
  const [r, setR] = useState<CircuitResult<'stubMatch'> | null>(null);
  const calc = async (next: CircuitInput<'stubMatch'>) => {
    setInput(next);
    const res = await run('stubMatch', next);
    if (res) setR(res);
  };
  useEffect(() => void calc(input), []);
  return (
    <Stack gap="sm">
      <Note>負荷から線路に沿って L1 の所に、同じ線路のスタブ（長さ L2）を並列に付けて整合します。半波長の中で SWR が 1.5 未満まで下がる位置をすべて示します。</Note>
      <SimpleGrid cols={3} spacing="xs">
        <ValueField label="周波数" unit="MHz" value={input.frequencyMhz} validate={positive} onCommit={(frequencyMhz) => calc({ ...input, frequencyMhz })} />
        <ValueField label="負荷 R" unit="Ω" value={input.load.re} validate={nonNegative} onCommit={(re) => calc({ ...input, load: { ...input.load, re } })} />
        <ValueField label="負荷 jX" unit="Ω" value={input.load.im} onCommit={(im) => calc({ ...input, load: { ...input.load, im } })} />
      </SimpleGrid>
      <CableFields
        cable={input.cable}
        name={cableName}
        onChange={(n, cable) => {
          setCableName(n);
          calc({ ...input, cable });
        }}
      />
      <ValueField label="整合先の R（給電線）" unit="Ω" value={input.referenceOhm} validate={positive} onCommit={(referenceOhm) => calc({ ...input, referenceOhm })} />
      {r && r.solutions.length === 0 && <Text size="sm">SWR が 1.5 未満になる位置がありません。</Text>}
      {r && r.solutions.length > 0 && r.solutions.every((s) => s.swr >= STUB_MATCHED_SWR) && (
        <Warning>
          この線路の Z₀ では整合しきれません（どの位置でも SWR {sig(Math.min(...r.solutions.map((s) => s.swr)), 3)} までしか下がりません）。線路の Z₀ を整合先の R に近づけてください。
        </Warning>
      )}
      {r && r.solutions.length > 0 && (
        <Table withTableBorder fz="xs" className="num">
          <Table.Thead>
            <Table.Tr>
              <Table.Th />
              {r.solutions.map((_, i) => (
                <Table.Th key={i} ta="right">
                  解 {i + 1}
                </Table.Th>
              ))}
            </Table.Tr>
          </Table.Thead>
          <Table.Tbody>
            {(
              [
                ['L1（負荷から）', (s) => `${sig(s.positionWl, 4)} λ / ${sig(s.positionCm, 4)} cm`],
                ['そこでの Z', (s) => `${sig(s.impedance.re, 4)} ${s.impedance.im < 0 ? '−' : '+'} j${sig(Math.abs(s.impedance.im), 4)} Ω`],
                ['スタブの X', (s) => (s.stub ? `${sig(s.stub.reactanceOhm, 4)} Ω（${elementText(s.stub.element)}）` : '不要')],
                ['L2 ショート', (s) => (s.stub ? `${sig(s.stub.shortWl, 4)} λ / ${sig(s.stub.shortCm, 4)} cm` : '—')],
                ['L2 開放', (s) => (s.stub ? `${sig(s.stub.openWl, 4)} λ / ${sig(s.stub.openCm, 4)} cm` : '—')],
                ['整合後の Z', (s) => `${sig(s.input.re, 4)} ${s.input.im < 0 ? '−' : '+'} j${sig(Math.abs(s.input.im), 4)} Ω`],
                ['SWR', (s) => sig(s.swr, 4)],
              ] as const satisfies readonly (readonly [string, (s: CircuitResult<'stubMatch'>['solutions'][number]) => string])[]
            ).map(([label, value]) => (
              <Table.Tr key={label}>
                <Table.Td>{label}</Table.Td>
                {r.solutions.map((s, i) => (
                  <Table.Td key={i} ta="right">
                    {value(s)}
                  </Table.Td>
                ))}
              </Table.Tr>
            ))}
          </Table.Tbody>
        </Table>
      )}
      <Note>位置は 0.0001 λ 刻みで探すので、SWR は 1 ちょうどにはなりません。cm は短縮率を掛けた物理長です。損失は含みません。</Note>
      <ErrorLine error={error} />
    </Stack>
  );
}

// ---------------------------------------------------------------- dialog ---

const TABS: [CircuitTab, string][] = [
  ['tank', 'LC 共振'],
  ['coil', '空芯コイル'],
  ['match', 'LC 整合'],
  ['stub', 'スタブ'],
  ['line', '分布定数線路'],
  ['twoSection', 'Q マッチ'],
  ['stubMatch', 'スタブ整合'],
];

export function CircuitTools() {
  const { open, tab, setOpen, setTab } = useCircuits();
  const seed = useSeed();
  return (
    <Modal opened={open} onClose={() => setOpen(false)} title="計算ツール" size="xl">
      <Tabs value={tab} onChange={(t) => t && setTab(t as CircuitTab)}>
        <Tabs.List mb="sm">
          {TABS.map(([id, label]) => (
            <Tabs.Tab key={id} value={id}>
              {label}
            </Tabs.Tab>
          ))}
        </Tabs.List>
        <Tabs.Panel value="tank">
          <TankPage seed={seed} />
        </Tabs.Panel>
        <Tabs.Panel value="coil">
          <CoilPage />
        </Tabs.Panel>
        <Tabs.Panel value="match">
          <MatchPage seed={seed} />
        </Tabs.Panel>
        <Tabs.Panel value="stub">
          <StubPage seed={seed} />
        </Tabs.Panel>
        <Tabs.Panel value="line">
          <LinePage seed={seed} />
        </Tabs.Panel>
        <Tabs.Panel value="twoSection">
          <TwoSectionPage seed={seed} />
        </Tabs.Panel>
        <Tabs.Panel value="stubMatch">
          <StubMatchPage seed={seed} />
        </Tabs.Panel>
      </Tabs>
      <Text size="xs" c="dimmed" mt="md">
        数値欄には式（例: 1/(2*3.14*7)）も入力できます。値は Enter かほかの欄へ移ったときに計算します。開くたびに、直前の計算結果の周波数・インピーダンスと SWR 基準の R から始めます。
      </Text>
    </Modal>
  );
}
