// The eager hazard in chunk B: earlier than every planned module in the root's expected order, so
// on-demand wrapping legitimately leaves it eager — its record-position interop trigger runs in
// chunk B's *body*. Once the lowering closes the A <-> B cycle through the forwarder's overlay,
// B's body evaluates before A's, and this eager `require_carrier()` call reads the not-yet-assigned
// CJS wrapper var from chunk A: `TypeError: require_carrier is not a function`. The emergent-cycle
// fixpoint must project the overlay edge and wrap it.
import carrier from '../a/carrier.cjs.js';
globalThis.__carried = carrier();
export const ready = true;
