import assert from 'node:assert/strict';
import { loadForm, loadPage } from './dist/main.js';

const form = await loadForm();
const publicExports = ['count', 'increment', 'transferForm', 'translations'];
if (globalThis.__configName === 'allow-extension') {
  assert.ok(Object.keys(form).length > publicExports.length);
  for (const name of publicExports) assert.ok(name in form);
} else {
  assert.deepEqual(Object.keys(form).sort(), publicExports);
}
assert.equal(form.count, 0);
form.increment();
assert.equal(form.count, 1);
assert.strictEqual(await loadForm(), form);

const page = await loadPage();
assert.deepEqual(page.default(), ['Transfer', 'transfer']);
