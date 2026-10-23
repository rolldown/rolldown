import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

const dist = path.resolve(import.meta.dirname, 'dist');
const output = fs
  .readdirSync(dist)
  .filter((file) => file.endsWith('.js'))
  .map((file) => fs.readFileSync(path.join(dist, file), 'utf-8'))
  .join('\n');

import { load, main } from './dist/main.js';
import { other } from './dist/another.js';
assert.strictEqual(main(), 1);
assert.strictEqual(await load(), 1);
assert.strictEqual(other, 1);
assert.deepStrictEqual(globalThis.log, []);
assert.ok(!output.includes('DEAD_'), output);
