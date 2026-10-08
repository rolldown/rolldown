// ESM entry. Its top-level `require_dup` takes the natural wrapper name of `dup.cjs`, so that
// wrapper is renamed. `require_dup$1` is a local of `dup.cjs` and `require_dup$2` is the parameter
// below: the wrapper must avoid both, since the `require()` call is printed inside `read`.
export const require_dup = 'outer';

export function read(require_dup$2) {
  return [require('./dup.cjs'), require_dup$2()];
}
