// The potential eager hazard in chunk B. On-demand wrapping leaves this record-position interop
// trigger in B's body, which is safe only while the forwarder's overlay does not manufacture an
// A -> B edge. Wrap-all conservatively defers it through `init_eagerhaz`.
import carrier from '../a/carrier.cjs.js';
globalThis.__carried = carrier();
export const ready = true;
