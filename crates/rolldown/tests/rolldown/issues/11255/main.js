import assert from 'node:assert';
import('./a/foo.js').then((a) => assert.strictEqual(a.default, 1));
import('./b/foo.js').then((b) => b);
