import assert from 'node:assert';
import { foreign, loadNamespace, segNamespace } from './dist/index.js';
import * as segNamespaceFacade from './dist/seg_namespace.js';
import { s_foreign } from './dist/seg_static.js';

// Code outside the bundle loads the facade of each entry (issue #11767).
assert.strictEqual(s_foreign, foreign);
assert.strictEqual(segNamespaceFacade.s_foreign, foreign);

// An `import()` in the bundle reads the namespace object of the entry from the `group` chunk. It
// is the same object as the static namespace import.
const namespace = await loadNamespace();
assert.strictEqual(namespace, segNamespace);
assert.deepStrictEqual(Object.keys(namespace), ['s_foreign']);
assert.strictEqual(namespace.s_foreign, foreign);
