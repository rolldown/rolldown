import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

const dist = path.join(import.meta.dirname, 'dist');
const files = fs.readdirSync(dist).filter((file) => file.endsWith('.js'));
const limited = globalThis.__configName === undefined;
if (limited) {
  // outer.js would be copied into a, b, c and d; it stays a file. Once it is a file, inner.js has
  // three carriers (outer, e, f), under the limit, so it is still inlined, and small.js (two
  // carriers) is inlined as well.
  assert.ok(files.includes('outer.js'), `outer.js stays a file: ${files}`);
  assert.ok(!files.includes('inner.js'), `inner.js is inlined: ${files}`);
  assert.ok(!files.includes('small.js'), `small.js is inlined: ${files}`);
}

globalThis.events = [];
for (const entry of ['a', 'b', 'c', 'd', 'e', 'f']) {
  await import(`./dist/${entry}.js`);
}
assert.deepStrictEqual(globalThis.events, [
  'A outer reads inner small',
  'B outer reads inner small',
  'C outer reads inner',
  'D outer reads inner',
  'E inner',
  'F inner',
]);
