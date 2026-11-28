import log from './log.cjs';
import defer * as ns from './cjs.js';
import { readFoo } from './user.js';
import defer * as wrapped from './wrapped.js';

// `o` is the deferred namespace object. A symbol key and `then` do not evaluate `cjs.js`.
const o = ns;
export const isObject = typeof o === 'object' && o !== null;
export const tag = Object.prototype.toString.call(o);
export const thenInO = 'then' in o;
export const before = [...log];
export const foo = readFoo();
export const after = [...log];
// No static access names `hidden`, so the export must stay in the CommonJS module.
const key = 'hid' + 'den';
export const dynamicRead = o[key];
export const keys = Object.keys(o);
let writeError;
try {
  o.foo = 1;
} catch (e) {
  writeError = e.constructor.name;
}
export { writeError };
export const local = ns.bar;
export { ns as exported };
export const wrappedBar = wrapped.get().bar;
