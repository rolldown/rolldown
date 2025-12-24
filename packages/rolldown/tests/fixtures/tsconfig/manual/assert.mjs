import assert from 'node:assert';
import Bar, { app as bar } from './dist/bar.js';
import Foo, { app as foo } from './dist/foo.js';

assert.strictEqual(bar, 'bar');
assert.strictEqual(foo, 'foo');
assert.ok(/\bb\(/.test(Bar.toString()));
assert.ok(/\bf\(/.test(Foo.toString()));
