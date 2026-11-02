import assert from 'node:assert';
import { x as assignment } from './assignment.cjs';
import { x as destructuring } from './destructuring.cjs';
import { x as logicalAssignment } from './logical_assignment.cjs';
import { x as loop } from './loop.cjs';
import { x as nested } from './nested.cjs';
import { x as property, y as computedProperty } from './property.cjs';
import { x as shadowed } from './shadowed.cjs';

assert.strictEqual(assignment ? 'truthy' : 'falsy', 'falsy');
assert.strictEqual(destructuring ? 'truthy' : 'falsy', 'falsy');
assert.strictEqual(logicalAssignment ? 'truthy' : 'falsy', 'falsy');
assert.strictEqual(loop ? 'truthy' : 'falsy', 'falsy');
assert.strictEqual(nested ? 'truthy' : 'falsy', 'falsy');
assert.strictEqual(property ? 'truthy' : 'falsy', 'truthy');
assert.strictEqual(computedProperty ? 'truthy' : 'falsy', 'truthy');
assert.strictEqual(shadowed ? 'truthy' : 'falsy', 'truthy');
