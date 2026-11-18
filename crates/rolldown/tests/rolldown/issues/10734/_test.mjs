import assert from 'node:assert';

// https://github.com/rolldown/rolldown/issues/10734
// Importing the dynamic entry used to throw before any user code ran:
//   SyntaxError: Export 'service_exports' is not defined in module
const { done } = await import('./dist/main.js');
const service = await done;

assert.strictEqual(service.entryValue, 'entrysharedchainAchainB');

assert.strictEqual((await service.loadChainA()).chainA, 'chainA');
assert.strictEqual((await service.loadShared()).shared, 'sharedchainA');
assert.strictEqual((await service.loadCyclic()).cyclic, 'cyclicchainBentrysharedchainAchainB');
