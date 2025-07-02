import {Foo, Bar, baz, b, bar, foo } from './dist/main.js'
import assert from 'node:assert'


assert.equal(Foo.name, 'Foo');
assert.equal(Bar.name, 'Foo');

assert.equal(baz.name, 'baz');
assert.equal(b.name, 'baz');

assert.equal(foo.name, 'foo');
assert.equal(bar.name, 'foo');


