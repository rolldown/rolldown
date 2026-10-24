import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

assert.ok(
  fs.existsSync(path.join(import.meta.dirname, 'dist/shared.js')),
  'a chunk with side effects stays a file without strict execution order',
);

globalThis.events = [];
globalThis.markers = [];
await import('./dist/a.js');
await import('./dist/b.js');

assert.deepStrictEqual(globalThis.events, [
  'S body',
  'A 0 this:undefined 1',
  'B 1 this:undefined 2',
]);
