import * as ns from './outer.js';
(globalThis.__events ??= []).push('b');
export const sharedDef = ns.vDef;
