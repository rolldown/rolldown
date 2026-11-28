import './b.js';
import defer * as ns1 from './a1.js';
import defer * as ns2 from './a2.js';

export const read = () => ns1.a1 + ns2.a2;
