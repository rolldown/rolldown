import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

const dist = path.resolve(import.meta.dirname, 'dist');
const output = fs
  .readdirSync(dist)
  .filter((file) => file.endsWith('.js'))
  .map((file) => fs.readFileSync(path.join(dist, file), 'utf-8'))
  .join('\n');

import { big, hex, small } from './dist/main.js';
assert.strictEqual(big, 'big');
assert.strictEqual(small, 'small');
assert.strictEqual(hex, 'hex');
assert.ok(!output.includes('DEAD_'));
