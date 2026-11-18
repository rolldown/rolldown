import assert from 'node:assert';
import plain from './dep.js';
import tagged from './dep.js' with { kind: 'x' };
import both from './dep.js' with { kind: 'x', other: 'y' };
import reordered from './dep.js' with { other: 'y', kind: 'x' };

assert.deepStrictEqual([plain, tagged, both, reordered], [1, 2, 3, 3]);
assert.strictEqual(both, reordered);
