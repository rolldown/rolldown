// Composition regression pin — a *partial* eager forwarder (one included hop to a wrapped definer,
// one tree-shaken excluded hop to another wrapped definer) sitting inside the emergent chunk cycle.
//
// This guards the B/C interaction: the forwarder owns the init of the binding it actually
// discharges (`pv`, an included hop → the B per-obligation rule), the excluded `export { unused }`
// hop stays silent (tree-shaking equivalence), and at the same time the forwarder's included hop
// closes the emergent A <-> B cycle that the fixpoint must wrap (the C rule). The two rules compose:
// the projection routes only the live included hop, the fixpoint converges, and the eager interop
// reader is deferred so nothing crashes. Expected green in both strict modes.
import './a/a-first.js';
import './b/eagerhaz.js';
import './e-first.js';
import { pv, marker } from './a/forwarder.js';
import { bv } from './b/definer_b.js';

globalThis.__result = { pv, bv, marker: marker(), carried: globalThis.__carried };
