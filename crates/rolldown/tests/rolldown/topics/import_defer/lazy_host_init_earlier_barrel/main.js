import './b.js';
import defer * as ns from './a.js';
import defer * as zns from './z.js';

export const read = () => ns.a;
export const readZ = () => zns.foo;
