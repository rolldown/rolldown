import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync } from 'node:fs';
for (const name of ['a', 'b', 'shared']) {
  const directory = new URL('./dist/' + name + '/', import.meta.url);
  mkdirSync(directory, { recursive: true });
  writeFileSync(new URL('probe.cjs', directory), 'module.exports = ' + JSON.stringify(name) + ';');
}
const a = (await import('./dist/a/entry.js')).default;
const b = (await import('./dist/b/entry.js')).default;
assert.equal(a.load('./probe.cjs'), 'a');
assert.equal(b.load('./probe.cjs'), 'b');
assert.equal(a.getRequire(), a.getRequire());
assert.notEqual(a.getRequire(), b.getRequire());
assert.equal(a.getRequire().cache, b.getRequire().cache);
assert.match(a.getRequire().resolve('./probe.cjs'), /[/\\]a[/\\]probe\.cjs$/);
