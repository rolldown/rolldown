// Repro for https://github.com/rolldown/rolldown/issues/10844
//
// `src/cycle-a.js` and `src/cycle-b.js` form an import cycle, and `cycle-b`
// dynamically imports `cycle-a` (a cycle member). Under `codeSplitting: false`
// that forces `cycle-a` to be wrapped, and `wrap_module_recursively` then wraps
// everything reachable from it - including `messages.js`, `messages-index.js`
// and the `m*.js` modules behind the `export *` barrel.
//
// Each `m*.js` holds nothing but a hoisted function declaration, so its `__esm`
// closure is empty. The `init_m*()` calls that `messages-index.js`'s
// `export * from` lowering emits are therefore no-ops and should carry
// `/* @__PURE__ */` so the default `dce-only` minify can drop them along with
// the empty wrappers they keep alive.
import { hub } from './src/cycle-a.js';
import { lazy } from './src/cycle-b.js';

if (hub() !== 'm1m2b') {
  throw new Error(`expected hub() to be 'm1m2b', got '${hub()}'`);
}

lazy().then((mod) => {
  if (mod.hub() !== 'm1m2b') {
    throw new Error(`expected lazily imported hub() to be 'm1m2b', got '${mod.hub()}'`);
  }
});
