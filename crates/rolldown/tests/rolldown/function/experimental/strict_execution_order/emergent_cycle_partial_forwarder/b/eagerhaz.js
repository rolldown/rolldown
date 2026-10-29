// Potential eager interop reader in chunk B. It stays in the chunk body in on-demand mode, where
// consumer-local routing avoids the reverse edge; wrap-all defers it until A's carrier is assigned.
import carrier from '../a/carrier.cjs.js';
globalThis.__carried = carrier();
export const ready = true;
