// `exports` is reserved in CommonJS output, so this entry's top-level `exports` is renamed. It must
// not become `exports$1`: that is the parameter below, which would capture the reference to the
// top-level binding inside `f`.
const exports = 'local-exports';

export function f(exports$1) {
  return [exports, exports$1];
}
