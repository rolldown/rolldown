import assert from 'node:assert/strict';

globalThis.orderExemptionLog = [];
const main = await import('./dist/main.js');

assert.deepEqual(globalThis.orderExemptionLog, ['setup', ['read', true]]);
await (await main.loadRoute()).loadSetup();
