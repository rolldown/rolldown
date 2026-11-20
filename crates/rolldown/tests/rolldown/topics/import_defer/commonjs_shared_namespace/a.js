// Only static accesses: this record keeps no deferred namespace object.
import defer * as ns from './x.cjs';

export const fooA = () => ns.foo;
export const selfA = () => ns.self();
