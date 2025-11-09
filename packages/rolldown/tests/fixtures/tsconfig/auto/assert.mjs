import assert from 'node:assert';
import Bar, { app as bar } from './dist/bar.js';
import Foo, { app as foo } from './dist/foo.js';

assert.strictEqual(bar, 'bar');
assert.strictEqual(foo, 'foo');
assert.ok(Bar.toString().includes('b('));
assert.ok(Foo.toString().includes('f('));
