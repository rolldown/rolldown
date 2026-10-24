import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

const dist = path.join(import.meta.dirname, 'dist');
// A bare import of a CommonJS module with effects binds nothing; it runs for the effect alone,
// so the helper stays a file.
assert.ok(!fs.readFileSync(path.join(dist, 'a.js'), 'utf8').includes('__share('));
assert.ok(fs.existsSync(path.join(dist, 'helper.js')));

globalThis.events = [];
await import('./dist/a.js');
await import('./dist/b.js');
await import('./dist/c.js');

assert.deepStrictEqual(globalThis.events.slice(-4), ['cjs body', 'A yes', 'B yes', 'C yes']);
