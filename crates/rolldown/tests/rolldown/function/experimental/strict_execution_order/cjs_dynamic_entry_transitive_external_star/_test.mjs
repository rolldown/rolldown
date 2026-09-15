import assert from 'node:assert';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);

globalThis.__externalStarLog = [];

const main = require('./dist/main.js');
const namespace = await main.loaded;
assert.strictEqual(namespace.x, 1);
assert.strictEqual(
  namespace.externalValue,
  42,
  'transitive external star exports must survive on the resolved dynamic-import namespace',
);
assert.deepStrictEqual(globalThis.__externalStarLog, ['target', 'reader:1']);

delete globalThis.__externalStarLog;
