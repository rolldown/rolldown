import assert from 'node:assert';
import {Test as T, test as t} from './lib'

class Test extends T {
}
assert.strictEqual(Test.name, "Test");
function test() {
}
assert.strictEqual(test.name, "test");
export {
  T,
  t,
}

