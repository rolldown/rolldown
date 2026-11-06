// With `codeSplitting: false`, `import()` is lowered to `Promise.resolve().then(() => ...)` in
// place. The parameter `Promise` must not capture the injected `Promise`.
import lib from './lib.cjs';
export { lib };
export function nested(Promise) {
  return import('./dep.js');
}
