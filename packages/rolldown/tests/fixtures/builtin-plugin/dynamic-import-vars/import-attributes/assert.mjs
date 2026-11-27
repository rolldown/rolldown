import assert from 'node:assert/strict';
import {
  alias,
  legacyAssert,
  relative,
  repeatedWith,
  withAndAssert,
  withoutAttributes,
} from './dist/main.js';

const expected = "export default 'module';";
assert.strictEqual((await relative('module')).default.trim(), expected);
assert.strictEqual((await alias('module')).default.trim(), expected);
assert.strictEqual((await withoutAttributes('module')).default, 'module');
assert.strictEqual((await repeatedWith('module')).default, 'module');
assert.strictEqual((await legacyAssert('module')).default.trim(), expected);
assert.strictEqual((await withAndAssert('module')).default.trim(), expected);
