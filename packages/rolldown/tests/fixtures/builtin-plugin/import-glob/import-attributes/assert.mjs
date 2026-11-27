import assert from 'node:assert/strict';
import { eager, lazy } from './dist/main.js';

const expected = "export default 'module';";
assert.strictEqual(eager['./files/module.js'].default.trim(), expected);
assert.strictEqual((await lazy['./files/module.js']()).trim(), expected);
