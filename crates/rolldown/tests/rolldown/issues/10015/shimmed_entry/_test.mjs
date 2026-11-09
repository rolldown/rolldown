import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const cjs = globalThis.__configName?.startsWith('cjs');
const main = cjs ? require('./dist/main.js') : await import('./dist/main.js');
const entry = cjs ? require('./dist/entry.js') : await import('./dist/entry.js');

assert.deepEqual(Object.keys(entry).sort(), ['a-b', 'default', 'load', 'unused', 'value']);
assert.deepEqual(Object.keys(main).sort(), ['load', 'observed']);
assert.deepEqual(main.observed, [undefined, undefined, 42]);
assert.equal(entry.default, undefined);
assert.equal(entry['a-b'], undefined);
assert.equal(entry.unused, undefined);
assert.equal(main.load, entry.load);
assert.equal(await entry.load(), true);
