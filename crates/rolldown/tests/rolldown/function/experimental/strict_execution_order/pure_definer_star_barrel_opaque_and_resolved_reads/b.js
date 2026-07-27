import * as ns from './barrel.js';

// Second entry keeping the barrel in a shared chunk (the split-consumer topology).
(globalThis.__events ??= []).push('b');

export const sharedDef = ns.vDef;
