// The radio-protection map: the ratio of the field to its limit on a
// horizontal plane, in a sequential ramp on a log scale from 1 % to 100 %,
// filled where the limit is exceeded, with contour lines at 10 % and 100 %.
// Points too close to a wire for the field to be trusted are hatched.
import { useEffect, useMemo } from 'react';
import { BufferAttribute, BufferGeometry, Color, DoubleSide, ShaderMaterial } from 'three';
import type { ExposureGrid, ExposureMap } from '../domain/exposure';

const vertexShader = /* glsl */ `
  attribute float level;
  attribute float unsure;
  varying float vLevel;
  varying float vUnsure;
  varying vec2 vWorld;
  void main() {
    vLevel = level;
    vUnsure = unsure;
    vWorld = position.xy;
    gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
  }
`;

const fragmentShader = /* glsl */ `
  uniform vec3 low;
  uniform vec3 high;
  uniform vec3 exceed;
  uniform vec3 line;
  uniform float hatch;
  varying float vLevel;
  varying float vUnsure;
  varying vec2 vWorld;

  // coverage of a contour line at level l, about width pixels wide
  float contour(float l, float width) {
    float d = abs(vLevel - l) / max(fwidth(vLevel), 1e-6);
    return 1.0 - smoothstep(width * 0.5, width * 0.5 + 1.0, d);
  }

  void main() {
    // log10 of the ratio: -2 (1 %) .. 0 (100 %)
    float t = clamp((vLevel + 2.0) / 2.0, 0.0, 1.0);
    vec4 c = vLevel >= 0.0 ? vec4(exceed, 0.55) : vec4(mix(low, high, t), t > 0.0 ? 0.15 + 0.55 * t : 0.0);
    c = mix(c, vec4(line, 0.9), contour(0.0, 2.0));
    c = mix(c, vec4(high, 0.8), 0.6 * contour(-1.0, 1.0));
    if (vUnsure > 0.5) {
      float stripe = step(0.5, fract((vWorld.x + vWorld.y) / hatch));
      c = mix(c, vec4(0.5, 0.5, 0.5, 0.6), stripe * 0.7);
    }
    if (c.a <= 0.004) discard;
    gl_FragColor = c;
    #include <tonemapping_fragment>
    #include <colorspace_fragment>
  }
`;

export const EXPOSURE_COLORS = { low: '#f6d860', high: '#e8833a', exceed: '#d6293c', line: '#8f1023' };

export function ExposurePlane({ grid, map }: { grid: ExposureGrid; map: ExposureMap }) {
  const geometry = useMemo(() => {
    const { n, x0, y0, step, z } = grid;
    const position = new Float32Array(n * n * 3);
    const level = new Float32Array(n * n);
    for (let i = 0; i < n * n; i++) {
      position[i * 3] = x0 + (i % n) * step;
      position[i * 3 + 1] = y0 + Math.floor(i / n) * step;
      position[i * 3 + 2] = z;
      level[i] = Math.min(3, Math.max(-6, Math.log10(Math.max(map.ratio[i], 1e-6))));
    }
    const index: number[] = [];
    for (let iy = 0; iy < n - 1; iy++) {
      for (let ix = 0; ix < n - 1; ix++) {
        const a = iy * n + ix;
        index.push(a, a + 1, a + n, a + 1, a + n + 1, a + n);
      }
    }
    const g = new BufferGeometry();
    g.setAttribute('position', new BufferAttribute(position, 3));
    g.setAttribute('level', new BufferAttribute(level, 1));
    g.setAttribute('unsure', new BufferAttribute(map.unsure, 1));
    g.setIndex(index);
    return g;
  }, [grid, map]);
  useEffect(() => () => geometry.dispose(), [geometry]);

  const material = useMemo(
    () =>
      new ShaderMaterial({
        uniforms: {
          low: { value: new Color(EXPOSURE_COLORS.low) },
          high: { value: new Color(EXPOSURE_COLORS.high) },
          exceed: { value: new Color(EXPOSURE_COLORS.exceed) },
          line: { value: new Color(EXPOSURE_COLORS.line) },
          hatch: { value: 1 },
        },
        vertexShader,
        fragmentShader,
        transparent: true,
        side: DoubleSide,
        // the wires through the plane stay in front of it
        depthWrite: false,
      }),
    [],
  );
  useEffect(() => () => material.dispose(), [material]);
  // stripes a few points wide
  material.uniforms.hatch.value = grid.step * 1.5;

  return <mesh geometry={geometry} material={material} renderOrder={-1} raycast={() => null} />;
}
