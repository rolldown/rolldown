import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import './dist/main.js';

assert.equal(globalThis.workerExecuted, undefined);
const mainCode = await readFile(new URL('./dist/main.js', import.meta.url), 'utf8');
assert.doesNotMatch(mainCode, /import\s*["']\.\/worker\.js["']/);
