// Hole 1 — an *eager* re-export barrel closes an emergent chunk cycle through its import overlay.
//
// The forwarder in chunk A stays eager (a pure `export { x } from` with no order-sensitivity of
// its own), so on-demand wrapping never plans it. But its retained re-export of the order-wrapped
// `definer` still makes the lowering emit `init_definer()` in A's body and register A's dependency
// on `init_definer` — a cross-chunk A -> B edge carried by an `OrderImportOverlay`, not by an
// `init_A` the projector can see. Together with the baseline B -> A edge (chunk B's `eagerhaz`
// eagerly requires chunk A's CJS carrier) that closes a real A <-> B cycle. Chunk B's body then
// runs before chunk A assigns its `var require_carrier`, so `eagerhaz`'s record-position
// `require_carrier()` reads the unassigned var — the C-class `require_* is not a function` startup
// crash, recurring because the projector skips every importer without its own ESM `init_*`.
//
// Source order pins the expected evaluation: a-first (A), the eager carrier reader (B), e-first
// (entry chunk), then the definer subtree (B). The entry-chunk-hosted e-first runs after the
// grouped chunks in the predicted order but before the definer subtree in source order, so
// `definer` deviates (premature) and joins the wrap plan — while `eagerhaz`, earlier than every
// planned module in this root's expected order, legitimately stays eager, and the pure forwarder
// is never order-sensitive so it stays eager too.
import './a/a-first.js';
import './b/eagerhaz.js';
import './e-first.js';
import { marker, pv } from './a/forwarder.js';

globalThis.__result = { pv, marker: marker(), carried: globalThis.__carried };
