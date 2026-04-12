import Foo from 'esm-pkg';
import assert from 'assert';
const instance = new Foo();
assert.strictEqual(instance.ok, true);
