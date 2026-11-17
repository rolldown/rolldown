import assert from 'node:assert/strict';
import { loadForm, loadPage } from './dist/main.js';

const form = await loadForm();
assert.deepEqual(Object.keys(form).sort(), ['count', 'increment', 'transferForm', 'translations']);
assert.equal(form.count, 0);
form.increment();
assert.equal(form.count, 1);
assert.strictEqual(await loadForm(), form);

const page = await loadPage();
assert.deepEqual(page.default(), ['Transfer', 'transfer']);
