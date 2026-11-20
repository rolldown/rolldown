import defer * as ns from './a.js';
import defer * as ns2 from './a2.js';
import './w.js';

export const read = () => ns.a + ns2.a2;
