import assert from 'node:assert';
import { readdirSync } from 'node:fs';

assert.deepStrictEqual(
  readdirSync(new URL('./dist', import.meta.url), { recursive: true })
    .filter((file) => file.endsWith('.js'))
    .sort(),
  ['rolldown-runtime.js', 'x/a.js', 'y/z/b.js'],
  'shared.js is inlined into both entries instead of becoming its own chunk',
);

globalThis.hrefs = [];
await import('./dist/x/a.js');
await import('./dist/y/z/b.js');

assert.strictEqual(globalThis.hrefs.length, 2);
assert.ok(
  /\/dist\/assets\/asset-[^/]+\.txt#hook$/.test(globalThis.hrefs[0]),
  `the resolveFileUrl hook resolves the asset into the output directory: ${globalThis.hrefs[0]}`,
);
assert.strictEqual(
  globalThis.hrefs[1],
  globalThis.hrefs[0],
  'both entries resolve the same asset URL from carriers in different directories',
);
