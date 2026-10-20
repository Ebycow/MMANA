// Friendly names for the generated OpenAPI schema (src/api/schema.d.ts,
// regenerate with `npm run gen:api`). Rust is the single source of truth.
import type { components } from './schema';

type S = components['schemas'];

export type Project = S['Project'];
export type Antenna = S['Antenna'];
export type Environment = S['Environment'];
export type Wire = S['Wire'];
export type Point3 = S['Point3'];
export type Segmentation = S['Segmentation'];
export type Source = S['Source'];
export type Load = S['Load'];
export type LoadKind = S['LoadKind'];
export type GroundKind = S['GroundKind'];
export type WireMaterial = S['WireMaterial'];
export type StackDirection = S['StackDirection'];
export type CalcOptions = S['CalcOptions'];
export type CalcResult = S['CalcResult'];
export type PatternCut = S['PatternCut'];
export type SweepPoint = S['SweepPoint'];
export type ParsedProject = S['ParsedProject'];
export type CalculationRequest = S['CalculationRequest'];
export type SweepRequest = S['SweepRequest'];
export type SweepResponse = S['SweepResponse'];
export type FarFieldRequest = S['FarFieldRequest'];
export type FarFieldResponse = S['FarFieldResponse'];
export type NearFieldRequest = S['NearFieldRequest'];
export type NearFieldResponse = S['NearFieldResponse'];
export type ElementKind = S['ElementKind'];
export type ElementOp = S['ElementOp'];
export type ElementRow = S['ElementRow'];
export type ElementEdit = S['ElementEdit'];
export type ElementEditRequest = S['ElementEditRequest'];
export type Problem = S['Problem'];
