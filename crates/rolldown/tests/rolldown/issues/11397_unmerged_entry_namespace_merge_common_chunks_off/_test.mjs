import assert from 'node:assert/strict';
import './dist/main.js';

const page = await globalThis.load[1]();
assert.deepEqual(Object.keys(page), ['TITLE']);
assert.equal(page.TITLE, 'LAYERS');
