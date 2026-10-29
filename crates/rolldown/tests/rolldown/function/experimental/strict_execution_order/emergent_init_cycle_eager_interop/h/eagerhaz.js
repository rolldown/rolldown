// The eager hazard: earlier than every planned module in the root's expected order, so on-demand
// wrapping legitimately leaves it eager — its record-position interop trigger runs in chunk H's
// *body*. Once the lowering closes the S <-> H cycle, H's body evaluates before S's, and this
// eager `require_carrier_cjs()` call reads the not-yet-assigned CJS wrapper var from chunk S:
// `TypeError: require_carrier_cjs is not a function`. The emergent-cycle fixpoint must wrap it.
import carrier from '../s/carrier.cjs.js';
globalThis.__carried = carrier();
export const ready = true;
