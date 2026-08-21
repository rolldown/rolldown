import assert from 'node:assert';

globalThis.__externalStarLog = [];

const main = await import('./dist/main.js');
const namespace = await main.loaded;
assert.strictEqual(namespace.x, 1);
assert.strictEqual(namespace.externalValue, 42);
assert.deepStrictEqual(globalThis.__externalStarLog, ['target', 'reader:1']);

delete globalThis.__externalStarLog;
