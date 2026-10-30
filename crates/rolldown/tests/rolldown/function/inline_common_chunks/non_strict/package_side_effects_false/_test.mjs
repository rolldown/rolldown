import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

const dist = path.join(import.meta.dirname, 'dist');
const inlined = fs.readFileSync(path.join(dist, 'a.js'), 'utf8').includes('__share(');
assert.strictEqual(
  inlined,
  !fs.existsSync(path.join(dist, 'lib.js')),
  'the lib chunk is either a file or inlined',
);

globalThis.events = [];
globalThis.markers = [];
await import('./dist/a.js');
await import('./dist/b.js');

// `sideEffects: false` lets the lib's body run after `after.js` once it is inlined.
assert.deepStrictEqual(
  globalThis.events,
  inlined ? ['after body', 'L body', 'A', 'B'] : ['L body', 'after body', 'A', 'B'],
);
assert.strictEqual(globalThis.markers[0], globalThis.markers[1]);
