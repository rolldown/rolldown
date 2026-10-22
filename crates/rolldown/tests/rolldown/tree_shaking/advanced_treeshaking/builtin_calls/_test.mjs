import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

const dist = path.resolve(import.meta.dirname, 'dist');
const output = fs
  .readdirSync(dist)
  .filter((file) => file.endsWith('.js'))
  .map((file) => fs.readFileSync(path.join(dist, file), 'utf-8'))
  .join('\n');

import { done } from './dist/main.js';
assert.strictEqual(done, 1);
assert.ok(!output.includes('DEAD_'), output);
assert.ok(!output.includes('123'), output);
