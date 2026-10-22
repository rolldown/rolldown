import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

const dist = path.resolve(import.meta.dirname, 'dist');
const output = fs
  .readdirSync(dist)
  .filter((file) => file.endsWith('.js'))
  .map((file) => fs.readFileSync(path.join(dist, file), 'utf-8'))
  .join('\n');

import { write } from './dist/main.js';
write(true);
write(false);
assert.deepStrictEqual(globalThis.log, ['written']);
assert.ok(!output.includes('DEAD_'));
