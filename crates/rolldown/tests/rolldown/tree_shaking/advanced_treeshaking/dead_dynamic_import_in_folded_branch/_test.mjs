import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

const dist = path.resolve(import.meta.dirname, 'dist');
const output = fs
  .readdirSync(dist)
  .filter((file) => file.endsWith('.js'))
  .map((file) => fs.readFileSync(path.join(dist, file), 'utf-8'))
  .join('\n');

import { load } from './dist/main.js';
assert.strictEqual(load(), null);
assert.ok(!output.includes('DEAD_'));
assert.deepStrictEqual(
  fs.readdirSync(dist).filter((file) => file.endsWith('.js')),
  ['main.js'],
);
