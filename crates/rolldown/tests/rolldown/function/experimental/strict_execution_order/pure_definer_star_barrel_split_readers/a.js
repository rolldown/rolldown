import * as ns from './barrel.js';

(globalThis.__events ??= []).push('a');

// This entry reads the pure definer's binding through the barrel namespace.
export const defValue = ns.vDef;
