import * as ns from './outer.js';
(globalThis.__events ??= []).push('a');
export const defValue = ns.vDef;
export const nsObject = ns;
