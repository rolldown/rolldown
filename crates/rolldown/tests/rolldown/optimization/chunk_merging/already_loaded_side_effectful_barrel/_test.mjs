import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';

globalThis.fixtureLog = [];
const log = globalThis.fixtureLog;

const entry = await import('./dist/entry.js');

assert.deepEqual(log, ['initialize', 'eager:value-b', 'entry:value-a']);
assert.equal(globalThis.fixtureInitialized, true);

await entry.loadPart('a');
assert.deepEqual(log, [
  'initialize',
  'eager:value-b',
  'entry:value-a',
  'lazy-a:value-a-1+value-a-2',
]);

await entry.loadPart('b');
assert.deepEqual(log, [
  'initialize',
  'eager:value-b',
  'entry:value-a',
  'lazy-a:value-a-1+value-a-2',
  'lazy-b:value-b',
]);

await entry.loadPart('c');
assert.deepEqual(log, [
  'initialize',
  'eager:value-b',
  'entry:value-a',
  'lazy-a:value-a-1+value-a-2',
  'lazy-b:value-b',
  'lazy-c:value-c',
]);

assert.equal(
  log.filter((value) => value === 'initialize').length,
  1,
  `package initialization must run exactly once; got ${JSON.stringify(log)}`,
);

const distDir = path.join(import.meta.dirname, 'dist');
const jsFiles = fs.readdirSync(distDir).filter((file) => file.endsWith('.js'));
for (const file of jsFiles) {
  const code = fs.readFileSync(path.join(distDir, file), 'utf8');
  assert.doesNotMatch(
    code,
    /unused-family-value/,
    `unused barrel family must be eliminated, found in dist/${file}`,
  );
}

assert.ok(
  !jsFiles.includes('library.js'),
  `expected the side-effectful barrel to fold into entry.js; got ${jsFiles.sort().join(', ')}`,
);

for (const file of ['lazy-consumer-a.js', 'lazy-consumer-b.js', 'lazy-consumer-c.js']) {
  assert.doesNotMatch(
    fs.readFileSync(path.join(distDir, file), 'utf8'),
    /["']\.\/library\.js["']/,
    `${file} should not import a barrel side effect already executed by entry.js`,
  );
}
