import log from './log.cjs';
import defer * as ns from './cjs.js';
import defer * as again from './cjs.js';
import defer * as mid from './mid.js';

// `then` is always absent, and reading it does not evaluate `cjs.js`.
export const thenValue = ns.then;
export const before = [...log];
export const value = ns.foo;
export const hasBar = 'bar' in again;
export const same = ns.default === again.default;
export const after = [...log];
// `mid.js` is wrapped, and it defers `cjs.js` too.
export const midValue = mid.read();
