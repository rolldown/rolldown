// Composition regression pin — a *partial* eager forwarder with one included hop to a wrapped
// definer and one tree-shaken excluded hop to another wrapped definer.
//
// This guards the B/C interaction: the forwarder owns the init of the binding it actually
// discharges (`pv`, an included hop → the B per-obligation rule), while the excluded
// `export { unused }` hop stays silent (tree-shaking equivalence). In on-demand mode the real
// consumer owns `init_definer`, so the retained path must not add a duplicate A -> B edge;
// wrap-all may conservatively route through the forwarder and defer the eager interop reader.
// Expected green in both strict modes.
import './a/a-first.js';
import './b/eagerhaz.js';
import './e-first.js';
import { pv, marker } from './a/forwarder.js';
import { bv } from './b/definer_b.js';

globalThis.__result = { pv, bv, marker: marker(), carried: globalThis.__carried };
