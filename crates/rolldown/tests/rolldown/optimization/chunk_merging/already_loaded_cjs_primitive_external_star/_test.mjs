import assert from 'node:assert';
import { createRequire } from 'node:module';
import { join } from 'node:path';

const require = createRequire(import.meta.url);
const main = require(join(import.meta.dirname, 'dist', 'main.js'));
const appNs = await main.app;

assert.deepEqual(Object.keys(appNs).sort(), ['0', '1', '2', 'done', 'own']);
assert.deepEqual([appNs[0], appNs[1], appNs[2]], ['a', 'b', 'c']);
assert.strictEqual(appNs.own, 43);
assert.strictEqual(await appNs.done, 84);
