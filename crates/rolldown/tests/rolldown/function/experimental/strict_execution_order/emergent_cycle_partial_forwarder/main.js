// Composition regression pin — a *partial* eager forwarder with one included hop to a wrapped
// definer and one tree-shaken excluded hop to another wrapped definer.
//
// This guards two rules together: the included `pv` hop belongs to its real consumer — the entry
// calls `init_definer` in both strict modes, so the forwarder's retained path must not add a
// duplicate A -> B edge — while the excluded `export { unused }` hop stays silent (tree-shaking
// equivalence). Wrap-all also defers the eager interop reader. Expected green in both strict modes.
import './a/a-first.js';
import './b/eagerhaz.js';
import './e-first.js';
import { pv, marker } from './a/forwarder.js';
import { bv } from './b/definer_b.js';

globalThis.__result = { pv, bv, marker: marker(), carried: globalThis.__carried };
