// Facade gate must consume post-lowering edges (rolldown/rolldown#10104 review item 1).
//
// Source order `a-first` (chunk A), `e-first` (this entry chunk), then `wrapper` (chunk A) makes
// `wrapper` deviate (premature) under the root, so on-demand wrapping order-wraps it. `wrapper`
// carries a tree-shaken `export * from './f.js'`; the excluded-statement metadata walks the dropped
// `f`'s `import { unused } from './gs/t.js'` and projects an `init_t` forward, so the analysis's
// post-lowering edges include `chunk A -> the sec entry's chunk`. That emergent edge has no
// baseline counterpart (f is excluded), so it is invisible to a facade gate keyed on pre-lowering
// edges — the exact miss this fix repairs.
import './a/a-first.js';
import './e-first.js';
import { wv } from './a/wrapper.js';
globalThis.__mainResult = wv;
