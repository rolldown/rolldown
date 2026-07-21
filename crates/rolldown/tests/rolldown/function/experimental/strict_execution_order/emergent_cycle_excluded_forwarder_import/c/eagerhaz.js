// The eager hazard in chunk C: earlier than every planned module in the first root's expected
// order, so on-demand wrapping legitimately leaves it eager — its record-position interop trigger
// runs in chunk C's *body*. Once the lowering closes the A <-> C cycle through the wrapped barrel's
// excluded hop, C's body evaluates before A's, and this eager `require_carrier()` call reads the
// not-yet-assigned CJS wrapper var from chunk A: `TypeError: require_carrier is not a function`.
import carrier from '../a/carrier.cjs.js';
globalThis.__carried = carrier();
export const ready = true;
