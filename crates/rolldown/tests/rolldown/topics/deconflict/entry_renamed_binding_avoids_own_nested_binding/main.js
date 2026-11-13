// The entry re-exports `other.js`'s `join`, and entry exports are named first, so this entry's
// top-level `join` is renamed. It must not become `join$1`: that is the parameter below, which would
// capture the reference to the top-level binding inside `f`.
export { join as otherJoin } from './other.js';

const join = 'local-join';

export function f(join$1) {
  return [join, join$1];
}
