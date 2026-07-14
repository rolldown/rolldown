// Eager interop reader in chunk B: its record-position require runs in chunk B's body. Once the
// forwarder's included hop closes the A <-> B cycle, the fixpoint must wrap this so the read waits
// for chunk A's carrier assignment.
import carrier from '../a/carrier.cjs.js';
globalThis.__carried = carrier();
export const ready = true;
