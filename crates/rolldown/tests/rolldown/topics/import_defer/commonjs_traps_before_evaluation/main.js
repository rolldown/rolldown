import log from './log.cjs';
import defer * as ns from './cjs.js';

// `cjs.js` is a CommonJS module, so the deferred namespace gets its keys at the first evaluation.
// Before it, these operations must give the results of a namespace that is not extensible and has
// a null prototype, and they must not evaluate `cjs.js`.
const o = ns;
const key = Symbol.for('key');
export const defineThen = Reflect.defineProperty(o, 'then', { value: () => 'then' });
export const defineSymbol = Reflect.defineProperty(o, key, { value: 1 });
export const symbolInO = key in o;
export const thenType = typeof Reflect.get(o, 'then');
export const sameTag = Reflect.defineProperty(o, Symbol.toStringTag, { value: 'Deferred Module' });
export const otherTag = Reflect.defineProperty(o, Symbol.toStringTag, { value: 'Module' });
export const setProto = Reflect.setPrototypeOf(o, {});
export const setProtoNull = Reflect.setPrototypeOf(o, null);
export const protoIsNull = Object.getPrototypeOf(o) === null;
export const before = [...log];
// The first evaluation.
export const keys = Object.keys(o);
export const after = [...log];
export const defineThenAfter = Reflect.defineProperty(o, 'then', { value: () => 'then' });
export const setProtoAfter = Reflect.setPrototypeOf(o, {});
