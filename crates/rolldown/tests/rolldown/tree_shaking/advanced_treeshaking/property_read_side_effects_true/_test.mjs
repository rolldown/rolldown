import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

const output = fs.readFileSync(path.resolve(import.meta.dirname, 'dist/main.js'), 'utf-8');

import { done } from './dist/main.js';
assert.strictEqual(done, true);
assert.ok(!output.includes('DEAD_'));
