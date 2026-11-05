import defer * as ns from './x.js';
import { log } from './log.js';

export const before = [...log];
// `then` of a deferred namespace is always undefined, and reading it evaluates nothing.
export const thenValue = ns.then;
export const thenIn = 'then' in ns;
export const afterThen = [...log];
// A bare read evaluates the module.
ns.foo;
export const afterRead = [...log];
export const fooIn = 'foo' in ns;
export const value = ns.foo + ns.bar;
// `y.js` exports no `then`: no missing export warning.
import defer * as plain from './y.js';

export const plainThen = plain.then;
export const afterPlainThen = [...log];
export const inner = plain.obj.inner;
export const afterInner = [...log];
