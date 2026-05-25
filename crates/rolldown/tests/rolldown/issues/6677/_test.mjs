import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

const distDir = path.join(import.meta.dirname, 'dist');
const assets = Object.fromEntries(
  fs.readdirSync(distDir).map((file) => [file, fs.readFileSync(path.join(distDir, file), 'utf8')]),
);

assert(!('components.js' in assets), 'the empty barrel facade should not be emitted');
for (const [file, code] of Object.entries(assets)) {
  assert(
    !code.includes('./components.js'),
    `${file} should not import the eliminated barrel facade`,
  );
}
assert(
  assets['index.js'].includes('./bundle-side.js'),
  'side-effectful dependencies forwarded by the barrel should be retargeted to importers',
);

globalThis.__rolldownIssue6677SideEffects = [];
await import('./dist/index.js');
await import('./dist/index-1.js');
assert.deepStrictEqual(globalThis.__rolldownIssue6677SideEffects, ['side']);
