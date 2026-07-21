// Hole 2 — a wrapped barrel's *excluded* `export * from` hop routes init through a non-included
// forwarder's plain static import, an edge the projector never resolves.
//
// The wrapped barrel `wrapper` (chunk A) carries a tree-shaken `export * from '../f.js'`. The real
// excluded-statement metadata walks EVERY static import of the non-included `f` — including its
// `import { unused } from '../c/t.js'` — and registers `init_t`, so the lowering makes chunk A
// import `init_t` from chunk C (A -> C). The fixpoint projector instead resolves that same hop with
// `collect_wrapped_esm_init_targets_for_import_record`, which follows `f`'s resolved *exports*;
// `unused` is imported by `f` but never re-exported, so it is not among them and the projector
// misses A -> C. Together with the baseline C -> A edge (chunk C's `eagerhaz` eagerly requires chunk
// A's CJS carrier) that is an undetected emergent A <-> C cycle: chunk C's body runs before chunk A
// assigns its `var require_carrier`, so `eagerhaz`'s record-position `require_carrier()` reads the
// unassigned var — the C-class `require_* is not a function` startup crash.
//
// `t` is imported directly (last) only so it is a live, order-wrapped module: its own source
// position, reached first through the `wrapper` subtree after the entry-chunk `e-first`, makes it
// deviate (premature) and join the wrap plan. That direct import adds an entry-chunk -> C edge but
// never reveals the A -> C hop, so the A <-> C cycle stays invisible to the projector, while
// `eagerhaz` — earlier than every planned module in the expected order — legitimately stays eager.
import './a/a-first.js';
import './c/eagerhaz.js';
import './e-first.js';
import { wv } from './a/wrapper.js';
import { tv } from './c/t.js';

globalThis.__result = { wv, tv, carried: globalThis.__carried };
