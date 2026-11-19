import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';

globalThis.entryLog = [];
const log = globalThis.entryLog;

await import('./dist/entry.js');

assert.deepEqual(log, ['entry:value-a']);

const namespace = await globalThis.loadD();
assert.deepEqual(log, ['entry:value-a', 'y:value-a', 'd:value-q']);
assert.deepEqual(Object.keys(namespace), []);

// Re-importing through the dynamic entry must not re-run any module.
await globalThis.loadD();
assert.deepEqual(log, ['entry:value-a', 'y:value-a', 'd:value-q']);

const distDir = path.join(import.meta.dirname, 'dist');
const jsFiles = fs.readdirSync(distDir).filter((file) => file.endsWith('.js'));

// The shared binding has exactly one definition.
const definitions = jsFiles
  .map((file) => fs.readFileSync(path.join(distDir, file), 'utf8'))
  .filter((code) => code.includes('"value-a"')).length;
assert.equal(definitions, 1, 'const a must be defined in exactly one chunk');
