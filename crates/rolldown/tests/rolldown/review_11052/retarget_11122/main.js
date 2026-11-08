// `other.js`'s `join` is an entry export, so it is named first and this entry's top-level `join`
// is renamed. It must not become `join$1`: that is the parameter below.
export { join as otherJoin } from './other.js';

const join = 'local-join';

export function f(join$1) {
  return [join, join$1];
}
