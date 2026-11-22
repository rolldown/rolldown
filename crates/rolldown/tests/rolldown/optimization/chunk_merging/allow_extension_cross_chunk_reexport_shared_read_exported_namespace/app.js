import * as other from './other.js';

// The entry exports the namespace of `counter_a.cjs`.
export * as lib from './counter_a.cjs';

// Code passes the namespace object of `other.js` to a function. `other.js` re-exports the
// namespace of `counter_b.cjs`.
const read = (namespace) => [namespace.lib.x, namespace.lib.y];
export const readOther = () => read(other);
