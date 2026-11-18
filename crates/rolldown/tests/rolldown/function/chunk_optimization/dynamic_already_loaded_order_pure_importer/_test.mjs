import assert from 'node:assert/strict';

const main = await import('./dist/main.js');

assert.equal(main.seen, 'effect,setup');
assert.equal(main.value, 'effect');
assert.equal((await main.loadSetup()).setupValue(), 'setup');
assert.equal((await main.loadBarrel()).value, 'effect');
