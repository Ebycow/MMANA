import { GizmoHelper, GizmoViewport, Grid, Html, Line, OrbitControls } from '@react-three/drei';
import { Canvas, useThree } from '@react-three/fiber';
import { ActionIcon, Group, Switch, Text, Tooltip, useComputedColorScheme } from '@mantine/core';
import { IconArrowAutofitHeight, IconArrowsMaximize, IconBox, IconLayoutDistributeHorizontal, IconSquare, IconViewfinder } from '@tabler/icons-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { Color, DoubleSide, MOUSE, Vector3 } from 'three';
import type { OrbitControls as OrbitControlsImpl } from 'three-stdlib';
import { transport } from '../api/transport';
import type { Point3, Project } from '../api/types';
import { feedPositions, pulsesFit, type FeedKind, type FeedPositions, type LayoutFor } from '../domain/feed';
import { bounds, currentPolylines, displayWires, reachingGround, sceneGrid, type Bounds, type DisplayWire, type SceneGrid } from '../domain/geometry';
import type { CameraAction } from '../domain/shortcuts';
import { add, niceStep, sub } from '../domain/wireEdit';
import { isDrawTool, useEditor } from '../state/editor';
import { useInput } from '../state/input';
import { useApp } from '../state/store';
import { EditScene } from './EditController';
import { EditToolbar, stepLabel } from './EditToolbar';
import { Panel } from './Panel';

// an unassigned button does nothing in OrbitControls
const CAMERA_MOUSE: Record<CameraAction, MOUSE> = { rotate: MOUSE.ROTATE, pan: MOUSE.PAN, zoom: MOUSE.DOLLY, none: -1 as MOUSE };

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

/**
 * The pulse layout of the project as last computed; null until the first one
 * arrives or when the engine cannot divide the wires.
 */
function usePulseLayout(): LayoutFor | null {
  const project = useApp((s) => s.project);
  const options = useApp((s) => s.options);
  const [latest, setLatest] = useState<LayoutFor | null>(null);
  useEffect(() => {
    let live = true;
    const frequencyMhz = options.frequencyMhz ?? project.antenna.frequencyMhz;
    transport.pulseLayout({ project, options }).then(
      (layout) => live && setLatest({ layout, project, frequencyMhz }),
      () => live && setLatest(null),
    );
    return () => {
      live = false;
    };
  }, [project, options]);
  return latest;
}

export function AntennaView() {
  const project = useApp((s) => s.project);
  const antenna = project.antenna;
  const environment = project.environment;
  const frequencyMhz = useApp((s) => s.options.frequencyMhz ?? s.project.antenna.frequencyMhz);
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
  /** Framing takes in the ground as well as the antenna. */
  const [fitGround, setFitGround] = useState(false);

  // a drag in progress shows its wires; the antenna changes when it ends
  const shown = useMemo(() => (preview ? { ...antenna, wires: preview } : antenna), [antenna, preview]);
  const wires = useMemo(() => displayWires(shown), [shown]);
  // framing and the grid step follow the antenna, not a drag in progress
  const box = useMemo(() => bounds(displayWires(antenna)), [antenna]);
  const autoStep = niceStep(box.radius / 25);
  const groundZ = environment.ground === 'freeSpace' ? null : -environment.height;
  const grid = useMemo(() => sceneGrid(box, groundZ ?? 0, step ?? autoStep), [box, groundZ, step, autoStep]);
  const frame = useMemo(() => (fitGround && groundZ != null ? reachingGround(box, groundZ) : box), [fitGround, groundZ, box]);
  const currents = useMemo(
    () => (result && showCurrents && !stale && !preview ? currentPolylines(wires, result) : null),
    [wires, result, showCurrents, stale, preview],
  );
  // the first stacked copy is the one edited; the others follow it
  const offset: Point3 = wires.length && shown.wires.length ? sub(wires[0].start, shown.wires[0].start) : { x: 0, y: 0, z: 0 };
  const latestLayout = usePulseLayout();
  const layout = latestLayout && pulsesFit(latestLayout, project, frequencyMhz) ? latestLayout : null;
  const feeds = useMemo(() => feedPositions(antenna, layout, shown.wires), [antenna, layout, shown.wires]);

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
          <Tooltip label={groundZ == null ? '大地も画面に収める（自由空間では使えません）' : `大地も画面に収める（${fitGround ? 'オン' : 'オフ'}）`}>
            <ActionIcon
              aria-label="大地も画面に収める"
              aria-pressed={fitGround}
              variant={fitGround && groundZ != null ? 'light' : 'subtle'}
              disabled={groundZ == null}
              onClick={() => {
                setFitGround(!fitGround);
                go(preset.p);
              }}
            >
              <IconArrowAutofitHeight size={15} />
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
          <Scene
            wires={wires}
            box={box}
            frame={frame}
            currents={currents}
            preset={preset}
            scheme={scheme}
            grid={grid}
            ground={environment.ground}
            feeds={feeds}
            offset={offset}
          />
          {/* picking and snapping use the wires as they are, not a drag in progress */}
          <EditScene
            wires={antenna.wires}
            display={wires}
            offset={offset}
            step={step ?? autoStep}
            editable={editable}
            radius={box.radius}
            feeds={feeds}
            layout={layout}
          />
        </Canvas>
        <EditToolbar autoStep={autoStep} editable={editable} />
        <EditOverlay />
        <div className="view3d-legend">
          {currents && <CurrentLegend maxMa={currents.maxMa} ramp={scheme === 'dark' ? RAMP_DARK : RAMP_LIGHT} />}
          {stale && result && showCurrents && (
            <Text size="xs" c="dimmed">
              編集後は再計算すると電流分布を表示します
            </Text>
          )}
          <SceneLegend grid={grid} ground={environment.ground} box={box} />
        </div>
      </div>
    </Panel>
  );
}

const DRAW_HINTS: Record<string, string> = {
  wire: 'クリックで点を追加 ・ 始点をクリックで閉じる ・ 数字で長さ ・ Enter / Esc / 右クリックで終了',
  rect: '対角の 2 点をクリック',
  polygon: '中心、続いて頂点をクリック ・ 数字で半径',
};
const FEED_HINT = '給電点（球）・負荷（箱）をドラッグしてワイヤー上のパルスへ ・ Esc で取り消し';

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
    tool === 'feed' && !hud && FEED_HINT,
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

type GroundKind = Project['environment']['ground'];
const GROUND_LABEL: Record<GroundKind, string> = { freeSpace: '自由空間（Z = 0 の面）', perfect: '完全導体の大地', real: '実在大地' };
const GROUND_FILL: Record<GroundKind, string | null> = { freeSpace: null, perfect: '--view3d-perfect-ground', real: '--view3d-real-ground' };

const metres = (v: number) => `${Number(v.toFixed(3))} m`;

/** What the horizontal grid is, how far apart its lines are and how high the antenna stands. */
function SceneLegend({ grid, ground, box }: { grid: SceneGrid; ground: GroundKind; box: Bounds }) {
  const fill = GROUND_FILL[ground];
  // the height setting lifts Z = 0; the lowest wire may stand elsewhere
  const lowest = box.minZ - grid.z;
  return (
    <div className="num">
      {fill && (
        <div>
          地上高 {metres(-grid.z)}（Z = 0 の高さ）{Math.abs(box.minZ) > 1e-9 && lowest >= 0 && ` ・ 最も低い点 ${metres(lowest)}`}
        </div>
      )}
      {fill && lowest < -1e-9 && (
        <Text size="xs" c="red">
          ワイヤーが地面より {metres(-lowest)} 下にあります
        </Text>
      )}
      <div>
        {fill && <span className="view3d-ground-swatch" style={{ background: `var(${fill})` }} />}
        {GROUND_LABEL[ground]} ・ グリッド {stepLabel(grid.cell)}（太線 {stepLabel(grid.section)}）
      </div>
    </div>
  );
}

function CurrentLegend({ maxMa, ramp }: { maxMa: number; ramp: string[] }) {
  return (
    <div>
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
  box: Bounds;
  /** What the camera takes in when framing. */
  frame: Bounds;
  currents: ReturnType<typeof currentPolylines> | null;
  preset: { p: Preset; n: number };
  scheme: 'light' | 'dark';
  /** The ground, or the Z = 0 plane in free space. */
  grid: SceneGrid;
  ground: GroundKind;
  /** Source and load markers in wire coordinates. */
  feeds: FeedPositions;
  /** Where the first stacked copy is drawn: world = wire + offset. */
  offset: Point3;
}

function Scene({ wires, box, frame, currents, preset, scheme, grid, ground, feeds, offset }: SceneProps) {
  const selected = useApp((s) => s.selectedWires);
  const hover = useEditor((s) => (s.hover ? s.hover.wire : null));
  const controls = useRef<OrbitControlsImpl>(null);
  const { camera, invalidate, size } = useThree();
  const mouse = useInput((s) => s.mouse);
  const mouseButtons = useMemo(
    () => ({ LEFT: CAMERA_MOUSE[mouse.buttons.left], MIDDLE: CAMERA_MOUSE[mouse.buttons.middle], RIGHT: CAMERA_MOUSE[mouse.buttons.right] }),
    [mouse.buttons],
  );

  const colors = useMemo(
    () => ({ wire: cssVar('--wire'), selected: cssVar('--wire-selected'), grid: cssVar('--view3d-grid'), section: cssVar('--view3d-grid-section'), dimension: cssVar('--text-secondary') }),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [scheme],
  );
  const ramp = scheme === 'dark' ? RAMP_DARK : RAMP_LIGHT;
  const fill = GROUND_FILL[ground];
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const groundColor = useMemo(() => (fill ? cssVar(fill) : null), [fill, scheme]);
  const g = grid;
  const groundEdge: [number, number, number][] = [
    [g.center.x - g.half, g.center.y - g.half, g.z],
    [g.center.x + g.half, g.center.y - g.half, g.z],
    [g.center.x + g.half, g.center.y + g.half, g.z],
    [g.center.x - g.half, g.center.y + g.half, g.z],
    [g.center.x - g.half, g.center.y - g.half, g.z],
  ];

  // camera framing
  useEffect(() => {
    const c = frame.center;
    const dir = new Vector3(...PRESET_DIRS[preset.p]).normalize();
    const fov = ((camera as { fov?: number }).fov ?? 35) * (Math.PI / 180);
    const aspect = size.width / Math.max(1, size.height);
    const fit = frame.radius / Math.sin(Math.min(fov, fov * aspect) / 2);
    camera.position.set(c.x + dir.x * fit * 0.85, c.y + dir.y * fit * 0.85, c.z + dir.z * fit * 0.85);
    camera.up.set(0, 0, 1);
    controls.current?.target.set(c.x, c.y, c.z);
    controls.current?.update();
    invalidate();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [preset.n]);

  const feedHover = useEditor((s) => s.feedHover);
  const feedDrag = useEditor((s) => s.feedDrag);
  /** Markers in world coordinates; the one being dragged sits on the pulse it would go on. */
  const markers = useMemo(() => {
    const place = (kind: FeedKind, at: (Point3 | null)[]) =>
      at.flatMap((p, index) => {
        const dragged = feedDrag?.item.kind === kind && feedDrag.item.index === index;
        const q = dragged && feedDrag.to ? feedDrag.to.position : p;
        const hot = dragged || (feedHover?.kind === kind && feedHover.index === index);
        return q ? [{ p: add(q, offset), hot, index }] : [];
      });
    return { src: place('source', feeds.sources), loads: place('load', feeds.loads) };
  }, [feeds, offset, feedHover, feedDrag]);

  // hover and selection change the lines without changing the camera
  useEffect(() => invalidate());

  const markerSize = box.radius * 0.025;

  return (
    <>
      <ambientLight intensity={1.4} />
      <directionalLight position={[5, -5, 10]} intensity={1.2} />
      <OrbitControls
        ref={controls}
        makeDefault
        enableDamping={false}
        mouseButtons={mouseButtons}
        // a negative speed turns the zoom round
        zoomSpeed={mouse.reverseZoom ? -mouse.zoomSpeed : mouse.zoomSpeed}
        zoomToCursor={mouse.zoomToCursor}
        rotateSpeed={mouse.rotateSpeed}
        onChange={() => invalidate()}
      />

      {/* the ground (real / perfect ground) or the Z = 0 plane; lines on round coordinates, fixed in place */}
      <Grid
        position={[grid.center.x, grid.center.y, grid.z]}
        rotation={[Math.PI / 2, 0, 0]}
        args={[grid.half * 2, grid.half * 2]}
        cellSize={grid.cell}
        sectionSize={grid.section}
        cellColor={colors.grid}
        sectionColor={colors.section}
        cellThickness={0.7}
        sectionThickness={1.1}
        fadeDistance={1e12}
        fadeStrength={0}
        side={DoubleSide}
      />
      {groundColor && (
        <>
          {/* the ground itself: a tinted surface under the grid, so it reads as solid */}
          <mesh position={[g.center.x, g.center.y, g.z]} renderOrder={-1} raycast={() => null}>
            <planeGeometry args={[g.half * 2, g.half * 2]} />
            <meshBasicMaterial color={groundColor} transparent opacity={0.28} depthWrite={false} side={DoubleSide} />
          </mesh>
          <Line points={groundEdge} color={colors.section} lineWidth={1.2} raycast={() => null} />
          <HeightDimension at={box.center} groundZ={g.z} tick={box.radius * 0.06} color={colors.dimension} />
        </>
      )}

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

      {markers.src.map(({ p, hot, index }) => (
        <mesh key={`s${index}`} position={[p.x, p.y, p.z]} scale={hot ? 1.5 : 1}>
          <sphereGeometry args={[markerSize, 20, 14]} />
          <meshStandardMaterial color="#eb6834" />
        </mesh>
      ))}
      {markers.loads.map(({ p, hot, index }) => (
        <mesh key={`l${index}`} position={[p.x, p.y, p.z]} scale={hot ? 1.5 : 1}>
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

/** The height setting as a dimension from the ground up to Z = 0, below the middle of the antenna. */
function HeightDimension({ at, groundZ, tick, color }: { at: Point3; groundZ: number; tick: number; color: string }) {
  // the label stays near the antenna, where the view is usually framed
  const label = tick * 8;
  // a fixed parent: drei's default one changes once events connect, remounting the label mid-render
  const gl = useThree((s) => s.gl);
  const portal = useMemo(() => ({ current: gl.domElement.parentElement as HTMLElement }), [gl]);
  if (groundZ >= 0) return null;
  const { x, y } = at;
  const bar = (z: number): [number, number, number][] => [
    [x - tick, y, z],
    [x + tick, y, z],
  ];
  return (
    <>
      <Line points={[[x, y, groundZ], [x, y, 0]]} color={color} lineWidth={1} dashed dashSize={-groundZ / 40} gapSize={-groundZ / 60} raycast={() => null} />
      <Line points={bar(groundZ)} color={color} lineWidth={1.5} raycast={() => null} />
      <Line points={bar(0)} color={color} lineWidth={1.5} raycast={() => null} />
      <Html portal={portal} position={[x, y, Math.max(groundZ / 2, -label)]} center zIndexRange={[4, 0]} style={{ pointerEvents: 'none' }}>
        <div className="view3d-dim-label num">地上高 {metres(-groundZ)}</div>
      </Html>
    </>
  );
}
