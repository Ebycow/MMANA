// The horizontal plane of the 3D view, the ground or Z = 0 in free space,
// drawn by one shader: the ground's tint, lines on round world coordinates
// at the cell, ten, a hundred and a thousand times it, the finer ones fading
// out where they crowd together on screen, the X and Y axes in their colours
// so the origin stands out, and a fade towards the edge of a disc so the plane
// has no hard border.
import { useEffect, useMemo } from 'react';
import { Color, DoubleSide, ShaderMaterial, Vector2 } from 'three';
import type { SceneGrid } from '../domain/geometry';

const vertexShader = /* glsl */ `
  varying vec2 vWorld;
  void main() {
    vec4 w = modelMatrix * vec4(position, 1.0);
    vWorld = w.xy;
    gl_Position = projectionMatrix * viewMatrix * w;
  }
`;

const fragmentShader = /* glsl */ `
  uniform vec2 center;
  uniform float radius;
  uniform float cell;
  uniform float section;
  uniform vec3 cellColor;
  uniform vec3 sectionColor;
  uniform vec3 fillColor;
  uniform float fillOpacity;
  uniform vec3 xAxisColor;
  uniform vec3 yAxisColor;
  varying vec2 vWorld;

  // coverage of lines size apart and width pixels wide; lines closer than a
  // few pixels fade out rather than turning into a haze
  float lines(float size, float width) {
    vec2 r = vWorld / size;
    vec2 d = fwidth(r);
    vec2 g = abs(fract(r - 0.5) - 0.5) / d;
    float on = 1.0 - min(min(g.x, g.y) + 1.0 - width, 1.0);
    return on * (1.0 - smoothstep(0.1, 0.2, max(d.x, d.y)));
  }

  // coverage of the line where v = 0, width pixels wide at any zoom
  float axis(float v, float width) {
    return 1.0 - min(abs(v) / fwidth(v) + 1.0 - width, 1.0);
  }

  vec4 over(vec4 src, vec4 dst) {
    float a = src.a + dst.a * (1.0 - src.a);
    return vec4((src.rgb * src.a + dst.rgb * dst.a * (1.0 - src.a)) / max(a, 1e-5), a);
  }

  void main() {
    float fade = 1.0 - smoothstep(0.55, 1.0, distance(vWorld, center) / radius);
    vec4 c = vec4(fillColor, fillOpacity);
    c = over(vec4(cellColor, 0.75 * lines(cell, 0.7)), c);
    c = over(vec4(sectionColor, lines(section, 1.1)), c);
    c = over(vec4(sectionColor, lines(section * 10.0, 1.3)), c);
    c = over(vec4(sectionColor, lines(section * 100.0, 1.3)), c);
    c = over(vec4(xAxisColor, axis(vWorld.y, 1.8)), c);
    c = over(vec4(yAxisColor, axis(vWorld.x, 1.8)), c);
    c.a *= fade;
    if (c.a <= 0.002) discard;
    gl_FragColor = c;
    #include <tonemapping_fragment>
    #include <colorspace_fragment>
  }
`;

interface GroundGridProps {
  grid: SceneGrid;
  cellColor: string;
  sectionColor: string;
  /** The ground's tint; null in free space. */
  fill: string | null;
  /** The lines along the X axis (Y = 0) and the Y axis (X = 0). */
  xAxisColor: string;
  yAxisColor: string;
}

export function GroundGrid({ grid, cellColor, sectionColor, fill, xAxisColor, yAxisColor }: GroundGridProps) {
  const material = useMemo(
    () =>
      new ShaderMaterial({
        uniforms: {
          center: { value: new Vector2() },
          radius: { value: 1 },
          cell: { value: 1 },
          section: { value: 10 },
          cellColor: { value: new Color() },
          sectionColor: { value: new Color() },
          fillColor: { value: new Color() },
          fillOpacity: { value: 0 },
          xAxisColor: { value: new Color() },
          yAxisColor: { value: new Color() },
        },
        vertexShader,
        fragmentShader,
        transparent: true,
        side: DoubleSide,
        // wires lying in the plane draw over it, and a drawing grid in the same plane is not hidden by it
        depthWrite: false,
        polygonOffset: true,
        polygonOffsetFactor: 1,
        polygonOffsetUnits: 1,
      }),
    [],
  );
  useEffect(() => () => material.dispose(), [material]);

  const u = material.uniforms;
  u.center.value.set(grid.center.x, grid.center.y);
  u.radius.value = grid.half;
  u.cell.value = grid.cell;
  u.section.value = grid.section;
  u.cellColor.value.set(cellColor);
  u.sectionColor.value.set(sectionColor);
  u.fillColor.value.set(fill ?? cellColor);
  u.fillOpacity.value = fill ? 0.28 : 0;
  u.xAxisColor.value.set(xAxisColor);
  u.yAxisColor.value.set(yAxisColor);

  return (
    <mesh position={[grid.center.x, grid.center.y, grid.z]} material={material} renderOrder={-1} raycast={() => null}>
      <planeGeometry args={[grid.half * 2, grid.half * 2]} />
    </mesh>
  );
}
