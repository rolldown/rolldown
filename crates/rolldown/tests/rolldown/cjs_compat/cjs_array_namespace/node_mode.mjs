import assert from 'node:assert';
import * as esModule from './node_mode.cjs';

// Node mode ignores `__esModule`: `default` is the array itself.
assert.deepStrictEqual([...esModule.default], [1, '2']);
assert.equal(esModule.default.default, 'value');
assert.equal(Object.hasOwn(esModule, '0'), false);
