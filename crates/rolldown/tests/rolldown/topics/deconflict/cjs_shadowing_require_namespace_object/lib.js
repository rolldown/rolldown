// ESM module `require()`d by the CJS entry. Because it is require()d (not statically imported),
// rolldown wraps it with `__esmMin` and renders its namespace object `var lib_exports = ...` at
// chunk root, which the entry's closure reads via `(init_lib(), __toCommonJS(lib_exports))`.
export default function makeValue() {
  return 'lib-default';
}
export const named = 'lib-named';
