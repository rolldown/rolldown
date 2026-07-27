import * as ns from './barrel.js';

(globalThis.__events ??= []).push('a');

// The statically resolved read: puts definer1's chain on the record's retained path.
export const defValue = ns.vDef;
// The opaque namespace use: retains every export of the barrel, including definer2's `wDef`,
// which is read only through the namespace object at runtime.
export const nsObject = ns;
