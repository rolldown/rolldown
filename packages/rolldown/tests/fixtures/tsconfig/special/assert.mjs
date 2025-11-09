import assert from 'node:assert';
import Bar, { app as bar } from './dist/bar.js';
import Foo, { app as foo } from './dist/foo.js';

assert.strictEqual(bar, 'foo');
assert.strictEqual(foo, 'foo');
assert.ok(Bar.toString().includes('f('));
assert.ok(Foo.toString().includes('f('));
