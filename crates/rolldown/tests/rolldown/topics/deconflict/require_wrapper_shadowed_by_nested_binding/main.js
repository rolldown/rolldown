// `require('./dep.cjs')` prints as a call to the wrapper `require_dep()`, so a parameter named
// `require_dep` must not capture it.
export function read(require_dep) {
  return [require('./dep.cjs').v, require_dep];
}
