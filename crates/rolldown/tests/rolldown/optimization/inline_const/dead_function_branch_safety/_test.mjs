import assert from 'node:assert/strict';

globalThis.branchEffects = [];
const {
  a,
  enable,
  hoisted,
  shadowed,
  indirectLogical,
  indirectConditional,
  throwingCondition,
  nested,
  lexical,
} = await import('./dist/a.js');
const { b } = await import('./dist/b.js');
assert.equal(a(), 'live');
assert.deepEqual(globalThis.branchEffects, ['import', 'test', 'live branch', 'nullish', 'ternary']);
assert.equal(b(), 'live');
assert.equal(hoisted(), undefined);
assert.equal(shadowed(true), 'live');
assert.equal(shadowed(false), 'shadowed');
assert.equal(indirectLogical(), undefined);
assert.equal(indirectConditional(), undefined);
assert.throws(throwingCondition, { message: 'condition' });
assert.equal(nested(true), 'nested');
assert.equal(nested(false), 'neither');
assert.equal(lexical(), 'outer');
assert.equal(globalThis.branchEffects.at(-1), 'inner');
enable();
assert.equal(a(), 'mutable');
