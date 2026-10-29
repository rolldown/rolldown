import * as ns from './outer-barrel.js';

(globalThis.__events ??= []).push('a');

// Statically resolved namespace member read of the pure definer through both star hops.
export const defValue = ns.vDef;
