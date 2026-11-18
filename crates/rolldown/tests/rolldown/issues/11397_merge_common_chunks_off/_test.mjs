import assert from 'node:assert/strict';
import { loadForm } from './dist/main.js';

// `page.js` imports the namespace object of `form.js` from the chunk of `form.js`. That export
// must not become a key of the namespace that `import()` gives.
const form = await loadForm();
assert.deepEqual(Object.keys(form).sort(), ['count', 'loadPage']);

// `import * as form` and `import()` give the same namespace object.
const page = await form.loadPage();
assert.strictEqual(page.default(), form);
