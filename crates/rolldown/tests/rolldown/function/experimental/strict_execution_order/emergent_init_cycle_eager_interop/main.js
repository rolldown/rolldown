// Entry for the emergent-chunk-cycle shape. Source order pins the expected evaluation:
// s-first (chunk S), the eager interop reader (chunk H), e-first (entry chunk), then the barrel
// (chunk S). The entry-chunk-hosted e-first runs *after* the grouped chunks in the predicted
// order but *before* the barrel subtree in source order, so `pure` and `barrel` deviate
// (premature) and join the wrap plan — while `eagerhaz`, earlier than every planned module in
// this root's expected order, legitimately stays eager.
import './s/s-first.js';
import './h/eagerhaz.js';
import './e-first.js';
import { pv, bmark } from './s/barrel.js';
globalThis.__result = { pv, bmark, carried: globalThis.__carried };
