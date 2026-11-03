// `other.js` imports `join` from an external, which is named first, so this entry's top-level
// `join` is renamed. It must not become `join$1`: that is the parameter below, which would
// capture the reference to the top-level binding inside `f`.
import './other.js';

const join = 'local-join';

export function f(join$1) {
  return [join, join$1];
}
