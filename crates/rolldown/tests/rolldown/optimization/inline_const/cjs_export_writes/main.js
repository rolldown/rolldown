import assert from 'node:assert';
import { x as conditional } from './conditional.cjs';
import { x as constant } from './constant.cjs';
import { x as inFunction } from './in_function.cjs';
import { x as logicalAssignment } from './logical_assignment.cjs';
import { x as moduleExports } from './module_exports.cjs';
import { x as topLevelReturn } from './top_level_return.cjs';

assert.strictEqual(conditional ? 'truthy' : 'falsy', 'falsy');
assert.strictEqual(constant ? 'truthy' : 'falsy', 'truthy');
assert.strictEqual(inFunction ? 'truthy' : 'falsy', 'falsy');
assert.strictEqual(logicalAssignment ? 'truthy' : 'falsy', 'falsy');
assert.strictEqual(moduleExports ? 'truthy' : 'falsy', 'falsy');
assert.strictEqual(topLevelReturn ? 'truthy' : 'falsy', 'falsy');
