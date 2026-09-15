// Eager partial forwarder in chunk A. The exported function keeps it retained without making it
// order-sensitive (it stays eager). It has two re-export hops:
//   - `export { pv } from '../b/definer.js'` — pv is consumed through this forwarder, so the hop is
//     included. On-demand routing gives its `init_definer` obligation to the real consumer without
//     leaving a duplicate direct-wrapper reference on the retained path.
//   - `export { unused } from '../b/definer_b.js'` — nothing consumes this binding, so the hop is
//     tree-shaken (excluded). A legally dead pure hop must trigger nothing; the projection must not
//     route it, or it would over-init definer_b for pages consuming none of its bindings.
export function marker() {
  return 'F';
}

export { pv } from '../b/definer.js';
export { unused } from '../b/definer_b.js';
