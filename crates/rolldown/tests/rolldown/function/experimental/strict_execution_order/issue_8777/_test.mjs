import assert from 'node:assert';
import { Foo, fooClasses, getFooUtilityClass } from './dist/main.js';

assert.deepStrictEqual(Foo, { name: 'Foo', root: 'MuiFoo-root' });
assert.deepStrictEqual(fooClasses, { root: 'MuiFoo-root' });
assert.strictEqual(getFooUtilityClass('root'), 'MuiFoo-root');
