import assert from 'node:assert';
import { modules } from './dist/main.js';

assert.deepStrictEqual(Object.keys(modules).sort(), ['/src/dir/a.js', '/src/dir/b.js']);
assert.strictEqual(modules['/src/dir/a.js'].default, 'a');
assert.strictEqual(modules['/src/dir/b.js'].default, 'b');
