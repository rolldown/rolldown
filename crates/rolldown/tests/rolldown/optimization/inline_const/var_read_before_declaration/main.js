import assert from 'node:assert';

const reads = [flag];
const errors = [];
try {
  run();
} catch (error) {
  errors.push(error.name);
}
export var flag = true;
export var run = () => {};

assert.deepStrictEqual(reads, [undefined]);
assert.deepStrictEqual(errors, ['TypeError']);
