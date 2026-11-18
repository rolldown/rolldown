import assert from 'node:assert/strict';

const main = await import('./dist/main.js');

assert.equal(main.value, 'service');
assert.equal(globalThis.ready, true);
assert.equal((await main.loadSetup()).setupValue(), 'setup');
assert.equal((await main.loadRead()).readValue(), 'read');
assert.equal((await main.loadBase()).baseValue(), 'base');
assert.equal((await main.loadForward()).forwardValue(), 'forward');
assert.equal((await main.loadConsumer()).consumerValue(), 'consumer');
assert.equal((await main.loadSplash()).splash, 'serviceslug');
assert.equal(await main.loadBarrel(), 'service');
