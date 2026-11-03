// sideEffects:false (via root package.json); imports `eg`, defines derived
// initializers. Distinct from the module providing the export the consumer uses.
import { eg } from './eg.js';
export const AccountSettings = eg.object({ a: eg.string, n: eg.number });
export const AccountMeta = eg.object({ b: eg.string });
