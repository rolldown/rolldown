import defer * as p1 from './p1.js';
import defer * as p2 from './p2.js';
import defer * as a from './a.js';
import defer * as b from './b.js';

export const primitive = () => [p1.ns === p2.ns, p1.ns.default];
export const twoExternals = () => [a.ns === b.ns, a.ns.default === b.ns.default];
