import path from 'node:path';
import { pathToFileURL } from 'node:url';
import assert from 'node:assert';

// Same cyclic dynamic-entry shape as issue #9320, but only one side exposes
// exports. The optimizer must not merge the exporting entry into the
// non-exporting entry chunk, because `import('./a.js')` must observe an empty
// namespace even though `a.js` statically uses `b.js`.

const distDir = path.join(import.meta.dirname, 'dist');
await import(pathToFileURL(path.join(distDir, 'main.js')).href);
await globalThis.__9320_single_export_done;

assert.deepStrictEqual(
  Object.keys(globalThis.__9320_single_export_aNs).sort(),
  [],
  'a namespace must stay empty',
);
assert.deepStrictEqual(
  Object.keys(globalThis.__9320_single_export_bNs).sort(),
  ['bImpl'],
  'b namespace must expose only b.js exports',
);
