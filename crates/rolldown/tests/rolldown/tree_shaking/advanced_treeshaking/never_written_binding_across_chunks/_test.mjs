import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

const dist = path.resolve(import.meta.dirname, 'dist');
const output = fs
  .readdirSync(dist)
  .filter((file) => file.endsWith('.js'))
  .map((file) => fs.readFileSync(path.join(dist, file), 'utf-8'))
  .join('\n');

import { mount } from './dist/main.js';
import { flag } from './dist/another.js';
assert.throws(() => mount(), /server only/);
assert.strictEqual(flag, false);
assert.ok(!output.includes('DEAD_'));
