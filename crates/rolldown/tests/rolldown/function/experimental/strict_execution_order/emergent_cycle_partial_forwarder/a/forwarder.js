// Eager partial forwarder in chunk A. The exported function keeps it retained without making it
// order-sensitive (it stays eager). It has two re-export hops:
//   - `export { pv } from '../b/definer.js'` — pv is consumed through this forwarder, so the hop is
//     included and the forwarder discharges it: this is the included hop that closes the emergent
//     A <-> B cycle (its `init_definer` overlay makes chunk A import chunk B).
//   - `export { unused } from '../b/definer_b.js'` — nothing consumes this binding, so the hop is
//     tree-shaken (excluded). A legally dead pure hop must trigger nothing; the projection must not
//     route it, or it would over-init definer_b for pages consuming none of its bindings.
export function marker() {
  return 'F';
}

export { pv } from '../b/definer.js';
export { unused } from '../b/definer_b.js';
