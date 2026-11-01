import path from 'node:path';
import { pathToFileURL } from 'node:url';
import assert from 'node:assert';
const distDir = path.join(import.meta.dirname, 'dist');
await import(pathToFileURL(path.join(distDir, 'main.js')).href);
await globalThis.__11397r_done;
// Why this build has 4 chunks, and `main` has 3 (`form.js`, `main.js` and `page.js`):
//
// `form.js` and `page.js` both use runtime helpers (`__toESM` and `__commonJSMin`) for their
// CommonJS imports. On `main`, the runtime goes into the chunk of the dynamic entry `form.js`.
// That chunk then exports the helpers to `page.js` as `n` and `t`, so `import('./form.js')` gives
// the keys `['n', 't', 'value']`. Node gives `['value']`.
//
// Now the runtime does not go into a dynamic entry chunk when another chunk uses its helpers
// (`runtime_merge_preserves_dynamic_entry_namespace`). So the runtime gets its own chunk,
// `rolldown-runtime.js`. That is the fourth chunk, and `form.js` exports only `value`.
//
// Rollup 4.65.0 (with `@rollup/plugin-commonjs` 29.0.3) also gives `['value']`, with 3 chunks. It
// keeps its helper in the `form` chunk and exports a namespace object of `form.js`, which the
// importer reads: `import('./form-[hash].js').then(function (n) { return n.f; })`. Rolldown could
// get the same layout if the runtime placement also used the namespace extraction. It does not do
// that yet. See internal-docs/code-splitting/implementation.md#runtime-module-placement.
assert.deepStrictEqual(Object.keys(globalThis.__11397r_formNs).sort(), ['value']);
