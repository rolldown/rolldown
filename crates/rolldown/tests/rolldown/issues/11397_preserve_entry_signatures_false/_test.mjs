import assert from 'node:assert/strict';
import './dist/main.js';

const form = await globalThis.loadForm();
assert.deepEqual(Object.keys(form).sort(), ['count', 'increment', 'transferForm', 'translations']);

const page = await globalThis.loadPage();
assert.deepEqual(page.default(), ['Transfer', 'transfer']);
