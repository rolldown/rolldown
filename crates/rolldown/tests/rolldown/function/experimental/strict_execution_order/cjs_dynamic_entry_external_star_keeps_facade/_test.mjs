import assert from 'node:assert';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);

globalThis.__externalStarLog = [];
globalThis.__externalStarOwnKeys = 0;

const main = require('./dist/main.js');
const namespace = await main.loaded;
assert.strictEqual(namespace.x, 1);
assert.strictEqual(namespace.externalValue, 42);
assert.strictEqual(namespace[0], 'a');
assert.strictEqual(namespace[1], 'b');
assert.strictEqual(
  globalThis.__externalStarOwnKeys,
  1,
  'the facade should enumerate a duplicated external module only once',
);
assert.deepStrictEqual(globalThis.__externalStarLog, ['target', 'reader:1']);

delete globalThis.__externalStarLog;
delete globalThis.__externalStarOwnKeys;
