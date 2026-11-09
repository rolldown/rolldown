import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { url, workerUrl } from './dist/main.js';

assert.equal(await readFile(url, 'utf8'), 'asset contents\n');
assert.equal(
  await readFile(workerUrl, 'utf8'),
  "throw new Error('The worker asset must not execute in the importing module');\n",
);
