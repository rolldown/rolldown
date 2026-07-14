import * as scales from './outer-barrel.js';
import './cyc-a.js';

(globalThis.__events ??= []).push('reader');

// The barrel namespace is referenced ONLY inside this deferred function, through a computed member
// key — exactly like recharts' `getD3ScaleFromType`: `var scales = d3Scales; scales[realScaleType]()`.
// There is no top-level read of the namespace or of any individual member, so no individual
// `scaleLinear` facade is ever a named import: the whole namespace object is what's retained. Under
// strict execution order, importing the barrel namespace forwards this module's `init_*` to the
// outer barrel's `init_*`, and the outer barrel's `init_*` must in turn forward through the inner
// barrel to the side-effect-free definer's `init_*`. The regression drops that hop, so the
// definer's `unit` is never assigned.
export function getScale(name) {
  const scaleNs = scales;
  const factory = scaleNs[name];
  const built = factory();
  return built ? built.value : 'NO_UNIT';
}
