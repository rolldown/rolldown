import assert from 'node:assert';
import { modules } from './dist/main.js';

// The first target (`./src/a/*`) is used, so files come from `./src/a/dir`.
assert.deepStrictEqual(Object.keys(modules), ['/src/a/dir/a.js']);
assert.strictEqual(modules['/src/a/dir/a.js'].default, 'from-a');
