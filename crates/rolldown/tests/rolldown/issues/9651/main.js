// Repro for https://github.com/rolldown/rolldown/issues/9651
//
// `src/zod/external.js` is reached two ways: statically through
// `import * as z from './src/zod/index.js'` (path 1), and through a dynamic
// `import()` chain `main -> src/dyn/a.js -> src/dyn/b.js -> src/zod/external.js`
// (path 2). The dynamic import forces zod's side-effect-free barrel modules to
// be wrapped (lazy-init). With tree-shaking on, `external.js` itself is dropped
// (its namespace is never read — only `z.locales`, which resolves through to
// `locales.js`), yet the finalizer used to still emit an `init_external()` call
// for it while lowering the static `import * as z` barrel, hitting a wrapper
// symbol with no chunk assignment and panicking:
//
//   "init_external" is not in any chunk, which is unexpected
import * as z from './src/zod/index.js';
import('./src/dyn/a.js');

if (z.locales.en() !== 'en') {
  throw new Error("expected z.locales.en() to be 'en'");
}
