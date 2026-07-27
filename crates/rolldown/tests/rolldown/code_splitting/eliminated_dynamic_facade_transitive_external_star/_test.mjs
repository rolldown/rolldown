import assert from 'node:assert';

globalThis.__externalStarLog = [];

const live = await import('./dist/live.js');
assert.strictEqual(live.fromX, 7);
assert.strictEqual(live.externalValue, 42);

const main = await import('./dist/main.js');
const namespace = await main.loaded;
assert.strictEqual(namespace.x, 1);
assert.strictEqual(namespace.viaBarrel, 7);
assert.strictEqual(namespace.externalValue, 42);
assert.deepStrictEqual(globalThis.__externalStarLog, ['target', 'reader:1']);

delete globalThis.__externalStarLog;
