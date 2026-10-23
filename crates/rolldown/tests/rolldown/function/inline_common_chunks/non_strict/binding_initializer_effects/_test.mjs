import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

const dist = path.join(import.meta.dirname, 'dist');
const inlined = fs.readFileSync(path.join(dist, 'a.js'), 'utf8').includes('__share(');
assert.strictEqual(
  inlined,
  !fs.existsSync(path.join(dist, 'helper.js')),
  'the helper chunk is either a file or inlined',
);

globalThis.events = [];
await import('./dist/a.js');
await import('./dist/b.js');
await import('./dist/c.js');

// The helper's effects only initialize its own bindings (the CommonJS interop and `new Box`),
// so they may run in its factory once inlined, and still run once.
assert.deepStrictEqual(globalThis.events, [
  'box body',
  'after body',
  'cjs body',
  'new helper',
  'A cjs helper',
  'B cjs helper',
  'new c',
  'C cjs c',
]);
