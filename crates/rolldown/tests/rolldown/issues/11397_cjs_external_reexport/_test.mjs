import assert from 'node:assert/strict';
import path from 'node:path';
import { loadForm, loadPage } from './dist/main.js';

// In CJS output, `sep` is a property read of the external module. Such a re-export must not stop
// the namespace extraction, so `import()` gives only the public exports of `form.js`.
const form = await loadForm();
assert.deepEqual(Object.keys(form).sort(), [
  'count',
  'increment',
  'sep',
  'transferForm',
  'translations',
]);
assert.equal(form.sep, path.sep);
form.increment();
assert.equal(form.count, 1);

const page = await loadPage();
assert.deepEqual(page.default(), ['Transfer', 'transfer']);
