import { GizmoHelper, GizmoViewport, Grid, Line, OrbitControls } from '@react-three/drei';
import { Canvas, useThree } from '@react-three/fiber';
import { ActionIcon, Group, Switch, Text, Tooltip, useComputedColorScheme } from '@mantine/core';
import { IconArrowsMaximize, IconBox, IconLayoutDistributeHorizontal, IconSquare, IconViewfinder } from '@tabler/icons-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { Color, Vector3 } from 'three';
import type { OrbitControls as OrbitControlsImpl } from 'three-stdlib';
import type { Point3 } from '../api/types';
import { bounds, currentPolylines, displayWires, pulseLocation, type DisplayWire } from '../domain/geometry';
import { niceStep, sub } from '../domain/wireEdit';
import { isDrawTool, useEditor } from '../state/editor';
import { useApp } from '../state/store';
import { EditScene } from './EditController';
import { EditToolbar } from './EditToolbar';
import { Panel } from './Panel';

type Preset = 'iso' | 'top' | 'front' | 'side';
const PRESET_DIRS: Record<Preset, [number, number, number]> = {
  iso: [1, -1.15, 0.75],
  top: [0.0001, -0.0001, 1],
  front: [1, 0, 0.0001],
  side: [0, -1, 0.0001],
};

// Sequential ramp (reference palette, blue) for current magnitude.
// Light mode runs light -> dark; dark mode dark -> light, so low current recedes.
const RAMP_LIGHT = ['#86b6ef', '#5598e7', '#2a78d6', '#1c5cab', '#104281', '#0d366b'];
const RAMP_DARK = ['#184f95', '#1c5cab', '#2a78d6', '#5598e7', '#86b6ef', '#cde2fb'];

function rampColor(ramp: string[], t: number): Color {
  const x = Math.min(1, Math.max(0, t)) * (ramp.length - 1);
  const i = Math.min(ramp.length - 2, Math.floor(x));
  return new Color(ramp[i]).lerp(new Color(ramp[i + 1]), x - i);
}

const cssVar = (name: string) => getComputedStyle(document.documentElement).getPropertyValue(name).trim() || '#888';

export function AntennaView() {
  const antenna = useApp((s) => s.project.antenna);
  const environment = useApp((s) => s.project.environment);
  const result = useApp((s) => s.result);
  const stale = useApp((s) => s.resultStale);
  const showCurrents = useApp((s) => s.showCurrents);
  const setShowCurrents = useApp((s) => s.setShowCurrents);
  const fileName = useApp((s) => s.fileName);
  const editable = useApp((s) => !s.elementEditorOpen && !s.optimizer.running && !s.optimizer.dialogOpen && !s.optimizer.resultsOpen);
  const preview = useEditor((s) => s.preview);
  const step = useEditor((s) => s.step);
  const scheme = useComputedColorScheme('light');
  const [preset, setPreset] = useState<{ p: Preset; n: number }>({ p: 'iso', n: 0 });

  // a drag in progress shows its wires; the antenna changes when it ends
  const shown = useMemo(() => (preview ? { ...antenna, wires: preview } : antenna), [antenna, preview]);
  const wires = useMemo(() => displayWires(shown), [shown]);
  // framing and the grid step follow the antenna, not a drag in progress
  const box = useMemo(() => bounds(displayWires(antenna)), [antenna]);
  const autoStep = niceStep(box.radius / 25);
  const currents = useMemo(
    () => (result && showCurrents && !stale && !preview ? currentPolylines(wires, result) : null),
    [wires, result, showCurrents, stale, preview],
  );
  // the first stacked copy is the one edited; the others follow it
  const offset: Point3 = wires.length && shown.wires.length ? sub(wires[0].start, shown.wires[0].start) : { x: 0, y: 0, z: 0 };

  // re-frame when another antenna is loaded
  useEffect(() => setPreset((s) => ({ p: s.p, n: s.n + 1 })), [fileName]);

  const go = (p: Preset) => setPreset((s) => ({ p, n: s.n + 1 }));
  const viewButtons: { p: Preset; label: string; icon: React.ReactNode }[] = [
    { p: 'iso', label: '斜めから', icon: <IconBox size={15} /> },
    { p: 'top', label: '上から (XY)', icon: <IconSquare size={15} /> },
    { p: 'front', label: '正面から (YZ)', icon: <IconLayoutDistributeHorizontal size={15} /> },
    { p: 'side', label: '横から (XZ)', icon: <IconViewfinder size={15} /> },
  ];

  return (
    <Panel
      title="アンテナ形状"
      scroll={false}
      actions={
        <Group gap={4} wrap="nowrap">
          <Switch
            label="電流分布"
            checked={showCurrents}
            disabled={!result}
            onChange={(e) => setShowCurrents(e.currentTarget.checked)}
            mr="xs"
          />
          {viewButtons.map((b) => (
            <Tooltip key={b.p} label={b.label}>
              <ActionIcon aria-label={b.label} onClick={() => go(b.p)}>
                {b.icon}
              </ActionIcon>
            </Tooltip>
          ))}
          <Tooltip label="全体を表示">
            <ActionIcon aria-label="全体を表示" onClick={() => go(preset.p)}>
              <IconArrowsMaximize size={15} />
            </ActionIcon>
          </Tooltip>
        </Group>
      }
    >
      <div style={{ position: 'absolute', inset: 0, background: 'var(--view3d-bg)' }}>
        <Canvas
          key={scheme}
          frameloop="demand"
          camera={{ up: [0, 0, 1], fov: 35, near: 0.01, far: 100000, position: [10, -12, 8] }}
          dpr={[1, 2]}
        >
          <Scene wires={wires} box={box} currents={currents} preset={preset} scheme={scheme} groundZ={environment.ground === 'freeSpace' ? null : -environment.height} />
          {/* picking and snapping use the wires as they are, not a drag in progress */}
          <EditScene wires={antenna.wires} display={wires} offset={offset} step={step ?? autoStep} editable={editable} radius={box.radius} />
        </Canvas>
        <EditToolbar autoStep={autoStep} editable={editable} />
        <EditOverlay />
        {currents && (
          <CurrentLegend maxMa={currents.maxMa} ramp={scheme === 'dark' ? RAMP_DARK : RAMP_LIGHT} />
        )}
        {stale && result && showCurrents && (
          <Text size="xs" c="dimmed" style={{ position: 'absolute', left: 12, bottom: 10 }}>
            編集後は再計算すると電流分布を表示します
          </Text>
        )}
      </div>
    </Panel>
  );
}

const DRAW_HINTS: Record<string, string> = {
  wire: 'クリックで点を追加 ・ 始点をクリックで閉じる ・ 数字で長さ ・ Enter / Esc / 右クリックで終了',
  rect: '対角の 2 点をクリック',
  polygon: '中心、続いて頂点をクリック ・ 数字で半径',
};

/** Read-outs of the drag or drawing in progress, and the selection box. */
function EditOverlay() {
  const hud = useEditor((s) => s.hud);
  const typed = useEditor((s) => s.typed);
  const box = useEditor((s) => s.box);
  const tool = useEditor((s) => s.tool);
  const drafting = useEditor((s) => s.draft.length > 0);
  const cursor = useEditor((s) => s.cursor);
  const plane = useEditor((s) => s.draftPlane ?? s.cursor?.plane);
  const draw = isDrawTool(tool);
  const lines = [
    hud,
    typed && `入力: ${typed} m（Enter で確定）`,
    draw && cursor && `${plane ? `${plane.toUpperCase()} 面 ` : ''}(${[cursor.point.x, cursor.point.y, cursor.point.z].map((v) => Number(v.toFixed(4))).join(', ')})`,
    draw && !drafting && !hud && DRAW_HINTS[tool],
  ].filter(Boolean);
  return (
    <>
      {box && <div className="view3d-box" style={{ left: box.x0, top: box.y0, width: box.x1 - box.x0, height: box.y1 - box.y0 }} />}
      {lines.length > 0 && (
        <div className="view3d-hud num">
          {lines.map((l, i) => (
            <div key={i}>{l}</div>
          ))}
        </div>
      )}
    </>
  );
}

function CurrentLegend({ maxMa, ramp }: { maxMa: number; ramp: string[] }) {
  return (
    <div style={{ position: 'absolute', left: 12, bottom: 10, fontSize: 11, color: 'var(--text-secondary)' }}>
      <div style={{ marginBottom: 3 }}>電流の大きさ</div>
      <div style={{ width: 140, height: 8, borderRadius: 4, background: `linear-gradient(90deg, ${ramp.join(',')})` }} />
      <div className="num" style={{ display: 'flex', justifyContent: 'space-between', width: 140, marginTop: 2 }}>
        <span>0</span>
        <span>{maxMa.toPrecision(3)} mA</span>
      </div>
    </div>
  );
}

interface SceneProps {
  wires: DisplayWire[];
  box: ReturnType<typeof bounds>;
  currents: ReturnType<typeof currentPolylines> | null;
  preset: { p: Preset; n: number };
  scheme: 'light' | 'dark';
  groundZ: number | null;
}

function Scene({ wires, box, currents, preset, scheme, groundZ }: SceneProps) {
  const antenna = useApp((s) => s.project.antenna);
  const selected = useApp((s) => s.selectedWires);
  const hover = useEditor((s) => (s.hover ? s.hover.wire : null));
  const controls = useRef<OrbitControlsImpl>(null);
  const { camera, invalidate, size } = useThree();

  const colors = useMemo(
    () => ({ wire: cssVar('--wire'), selected: cssVar('--wire-selected'), grid: cssVar('--chart-grid'), axis: cssVar('--chart-axis') }),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [scheme],
  );
  const ramp = scheme === 'dark' ? RAMP_DARK : RAMP_LIGHT;

  // camera framing
  useEffect(() => {
    const c = box.center;
    const dir = new Vector3(...PRESET_DIRS[preset.p]).normalize();
    const fov = ((camera as { fov?: number }).fov ?? 35) * (Math.PI / 180);
    const aspect = size.width / Math.max(1, size.height);
    const fit = box.radius / Math.sin(Math.min(fov, fov * aspect) / 2);
    camera.position.set(c.x + dir.x * fit * 0.85, c.y + dir.y * fit * 0.85, c.z + dir.z * fit * 0.85);
    camera.up.set(0, 0, 1);
    controls.current?.target.set(c.x, c.y, c.z);
    controls.current?.update();
    invalidate();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [preset.n]);

  const markers = useMemo(() => {
    const src = antenna.sources.map((s) => pulseLocation(s.pulse, antenna.wires)).filter((p) => p != null);
    const loads = antenna.loads.map((l) => pulseLocation(l.pulse, antenna.wires)).filter((p) => p != null);
    return { src, loads };
  }, [antenna]);

  // hover and selection change the lines without changing the camera
  useEffect(() => invalidate());

  const markerSize = box.radius * 0.025;
  const gridSize = Math.max(10, Math.ceil(box.radius * 4));

  return (
    <>
      <ambientLight intensity={1.4} />
      <directionalLight position={[5, -5, 10]} intensity={1.2} />
      <OrbitControls ref={controls} makeDefault enableDamping={false} onChange={() => invalidate()} />

      {/* ground plane (real / perfect ground) or a reference grid at z = 0 */}
      <Grid
        position={[box.center.x, box.center.y, groundZ ?? Math.min(0, box.minZ)]}
        rotation={[Math.PI / 2, 0, 0]}
        args={[gridSize, gridSize]}
        cellSize={gridSize / 40}
        sectionSize={gridSize / 8}
        cellColor={colors.grid}
        sectionColor={colors.axis}
        fadeDistance={gridSize * 2.5}
        infiniteGrid
      />

      {/* picking is done in screen space by EditScene */}
      {wires.map((w, i) => {
        const isSel = selected.includes(w.base);
        const isHover = hover === w.base;
        const showPlain = !currents;
        return (
          <Line
            key={i}
            points={[
              [w.start.x, w.start.y, w.start.z],
              [w.end.x, w.end.y, w.end.z],
            ]}
            color={isSel ? colors.selected : colors.wire}
            lineWidth={isSel ? 4 : isHover ? 3.5 : showPlain ? 2.2 : 1}
            transparent={!showPlain && !isSel}
            opacity={!showPlain && !isSel ? 0.35 : 1}
            raycast={() => null}
          />
        );
      })}

      {currents?.lines.map((l) => (
        <Line
          key={`c${l.wire}`}
          points={l.points.map((p) => [p.x, p.y, p.z] as [number, number, number])}
          vertexColors={l.levels.map((t) => rampColor(ramp, t).toArray() as [number, number, number])}
          lineWidth={3.2}
          raycast={() => null}
        />
      ))}

      {markers.src.map((p, i) => (
        <mesh key={`s${i}`} position={[p.x, p.y, p.z]}>
          <sphereGeometry args={[markerSize, 20, 14]} />
          <meshStandardMaterial color="#eb6834" />
        </mesh>
      ))}
      {markers.loads.map((p, i) => (
        <mesh key={`l${i}`} position={[p.x, p.y, p.z]}>
          <boxGeometry args={[markerSize * 1.4, markerSize * 1.4, markerSize * 1.4]} />
          <meshStandardMaterial color="#6250d6" />
        </mesh>
      ))}

      <GizmoHelper alignment="bottom-right" margin={[56, 56]} onUpdate={() => invalidate()}>
        <GizmoViewport axisColors={['#e34948', '#1baf7a', '#2a78d6']} labelColor="#ffffff" />
      </GizmoHelper>
    </>
  );
}
