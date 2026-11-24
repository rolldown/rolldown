import assert from 'node:assert/strict';

const main = await import('./dist/main.js');

assert.equal(main.observed, true);
await (await main.loadRoute()).loadSetup();
