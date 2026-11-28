import './b.js';
import defer * as ns from './a.js';
import './z.js';

export const access = () => ns.foo;
