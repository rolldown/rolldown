import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { url } from './dist/main.js';

assert.equal(await readFile(url, 'utf8'), 'asset contents\n');
