import { log } from './log.js';
import defer * as ns from './imports-tla.js';

// The preparation of the deferred import evaluates `tla.js` before the body of this module.
export const before = [...log];
export const x = ns.x;
// After the preparation, the access evaluates `imports-tla.js` synchronously.
export const after = [...log];
