import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync, readdirSync } from 'node:fs';
for (const name of ['a', 'b', 'shared']) {
  const directory = new URL('./dist/' + name + '/', import.meta.url);
  mkdirSync(directory, { recursive: true });
  writeFileSync(new URL('probe.cjs', directory), 'module.exports = ' + JSON.stringify(name) + ';');
}
await import('./dist/a/entry.js');
await import('./dist/b/entry.js');
const a = globalThis.entryA;
const b = globalThis.entryB;
assert.equal(a, b);
assert.equal(a.load('./probe.cjs'), 'shared');
assert.equal(b.load('./probe.cjs'), 'shared');
assert.equal(a.getRequire(), b.getRequire());
assert.ok(readdirSync(new URL('./dist/shared/', import.meta.url)).includes('shared.js'));
