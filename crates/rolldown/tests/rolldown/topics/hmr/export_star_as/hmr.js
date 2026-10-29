import assert from 'node:assert';
import { NS } from './reexport.js';

// `export * as NS from './dep.js'` must export the namespace object under `NS` -
// not spread `./dep.js`'s exports onto this module like `export *` does.
assert.ok(NS, 'NS should be a namespace object');
assert.strictEqual(NS.value, 'from-dep');

import.meta.hot.accept();
