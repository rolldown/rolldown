import assert from 'node:assert/strict';

const { ifStatement, conditional, logical } = await import('./dist/main.js');
assert.equal(ifStatement(), 'consequent');
assert.equal(conditional(), 'consequent');
assert.equal(logical(), 'right');
assert.equal(globalThis.topLevel, 'consequent');
