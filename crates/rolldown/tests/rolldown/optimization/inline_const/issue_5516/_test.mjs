import assert from 'node:assert';
import _default, {rrrr} from './dist/main.js'

assert.strictEqual(_default, 'test');
assert.strictEqual(rrrr, 1000);
