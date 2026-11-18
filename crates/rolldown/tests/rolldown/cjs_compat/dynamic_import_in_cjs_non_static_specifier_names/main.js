// With `dynamicImportInCjs: false`, a non-static `import(expr)` is lowered to
// `Promise.resolve().then(() => __toESM(require(expr)))`. The references in `expr` must still get
// the names that the finalizer prints for them.
import { target as aliased } from './target.js';
import cjsRoot from './cjs-root.cjs';

// The import binding `aliased` is printed as `target`.
export const viaImport = () => import(aliased);

// The lowered `import()` prints `Object`, so the parameter `Object` can get a new name.
export function viaParam(Object) {
  return import(Object);
}

export { cjsRoot };
