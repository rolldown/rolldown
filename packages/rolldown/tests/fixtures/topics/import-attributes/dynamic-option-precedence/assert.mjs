import assert from 'node:assert/strict';
import { imports, loadExternal, loadWithAccessors, loadWithBoth } from './dist/main.js';

const [plain, repeatedAssert, assertFirst, withFirst] = await imports();
assert.strictEqual(plain.default, 'module');
const text = "export default 'module';";
assert.strictEqual(repeatedAssert.default.trim(), text);
assert.strictEqual(assertFirst.default.trim(), text);
assert.strictEqual(withFirst.default.trim(), text);
assert.strictEqual((await loadExternal()).default, 42);
assert.strictEqual((await loadWithAccessors('data:application/json,42')).default, 42);
assert.strictEqual((await loadWithBoth('data:application/json,42')).default, 42);
