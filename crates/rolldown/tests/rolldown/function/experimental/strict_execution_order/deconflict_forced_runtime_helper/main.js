// Deconflict must register force-included runtime statements (rolldown/rolldown#10104 review item 2).
//
// A single-chunk strict build (codeSplitting disabled) places the runtime module next to user code
// (order_wrapping.rs:451-457). Pure ESM demands no runtime helper at link time, so tree-shaking drops
// `__esmMin`; strict order lowering then force-includes it. `helper.js` declares a top-level
// `__esmMin` of its own, hoisted to a root-scope `var __esmMin` by its order wrapper — the same root
// scope as the runtime's forced `__esmMin`. deconflict_chunk_symbols filtered the runtime module's
// statements on raw `stmt_info_included`, so the force-included helper statement never reached the
// renamer and the collision went unnoticed: `helper`'s init overwrites the shared binding with the
// user string, and the next wrapper's `__esmMin(...)` call throws `__esmMin is not a function`.
import { helper } from './helper.js';
import { late } from './late.js';
globalThis.__result = { helper, late };
