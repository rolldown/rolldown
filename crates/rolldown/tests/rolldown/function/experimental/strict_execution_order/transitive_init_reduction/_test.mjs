import assert from 'node:assert';
import fs from 'node:fs/promises';

const content = await fs.readFile(new URL('dist/main.js', import.meta.url), 'utf8');

const countCalls = (name) => (content.match(new RegExp(`${name}\\(\\)`, 'g')) || []).length;

// init_a: declared once + called once in entry = 1 call
assert.strictEqual(countCalls('init_a'), 1);
// init_b: declared once + called once inside init_a = 1 call
assert.strictEqual(countCalls('init_b'), 1);
// init_c: declared once + called once inside init_b = 1 call
assert.strictEqual(countCalls('init_c'), 1);
