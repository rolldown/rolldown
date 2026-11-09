import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';

globalThis.entryLog = [];
const log = globalThis.entryLog;

await import('./dist/entry.js');

// The entry chunk serves its re-export of `q` with a static import of the chunk
// owning `w`, so that chunk evaluates eagerly, before the entry body.
assert.deepEqual(log, ['y:value-a', 'd:value-q', 'entry:value-a']);

// Re-importing through the dynamic entry must not re-run any module.
await globalThis.loadD();
assert.deepEqual(log, ['y:value-a', 'd:value-q', 'entry:value-a']);

const distDir = path.join(import.meta.dirname, 'dist');
const jsFiles = fs.readdirSync(distDir).filter((file) => file.endsWith('.js'));

// `x` must stay in one chunk; folding it into the entry would require the cyclic
// import this fixture pins against.
const definitions = jsFiles
  .map((file) => fs.readFileSync(path.join(distDir, file), 'utf8'))
  .filter((code) => code.includes('"value-a"')).length;
assert.equal(definitions, 1, 'const a must be defined in exactly one chunk');

for (const file of jsFiles) {
  if (file === 'entry.js') continue;
  assert.doesNotMatch(
    fs.readFileSync(path.join(distDir, file), 'utf8'),
    /["']\.\/entry\.js["']/,
    `${file} must not import entry.js back; that static cycle reads uninitialized bindings`,
  );
}
