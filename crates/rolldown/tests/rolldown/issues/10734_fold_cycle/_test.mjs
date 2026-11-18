import assert from 'node:assert';

// https://github.com/rolldown/rolldown/issues/10734
const { done } = await import('./dist/main.js');
const service = await done;

assert.strictEqual(service.entryValue, 'entrysharedchainAchainB');

assert.strictEqual((await service.loadChainA()).chainA, 'chainA');
assert.strictEqual((await service.loadShared()).shared, 'sharedchainA');
assert.strictEqual((await service.loadCyclic()).cyclic, 'cyclicchainBentrysharedchainAchainB');
