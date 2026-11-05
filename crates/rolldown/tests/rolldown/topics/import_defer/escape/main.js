import { log } from './log.js';
import defer * as ns from './x.js';
import { x } from './other.js';

const use = (value) => value;
// One module has one deferred namespace, also through `export { x }`.
export const same = ns === x;
export const tag = Object.prototype.toString.call(use(ns));
export const before = [...log];
// `Object.keys` evaluates the module. The escaping namespace keeps all exports.
export const keys = Object.keys(use(ns));
export const after = [...log];
x.inc();
export const count = use(ns).count;
