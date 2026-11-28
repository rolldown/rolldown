import assert from 'node:assert';
import path from 'node:path';

globalThis.__extLog = [];
const main = await import('./dist/main.js');
assert.deepStrictEqual(globalThis.__extLog, ['main']);

// The first access evaluates `d.js`.
assert.strictEqual(main.read(), path.join('a', 'b'));
assert.deepStrictEqual(globalThis.__extLog, ['main', 'd']);

delete globalThis.__extLog;
