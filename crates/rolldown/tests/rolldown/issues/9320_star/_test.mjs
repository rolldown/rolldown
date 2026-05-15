import path from 'node:path';
import { pathToFileURL } from 'node:url';
import assert from 'node:assert';

// Same cyclic dynamic-entry shape as issue #9320, but the entry modules expose
// their namespace keys only through `export *`. This ensures the optimizer's
// export-pollution guard uses linked exports, not only syntactic named exports.

const distDir = path.join(import.meta.dirname, 'dist');
await import(pathToFileURL(path.join(distDir, 'main.js')).href);
await globalThis.__9320_star_done;

assert.deepStrictEqual(Object.keys(globalThis.__9320_star_formNs).sort(), ['formImpl']);
assert.deepStrictEqual(Object.keys(globalThis.__9320_star_actionNs).sort(), ['actionImpl']);
