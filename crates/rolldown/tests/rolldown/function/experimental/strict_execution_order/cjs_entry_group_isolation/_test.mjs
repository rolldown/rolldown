import assert from 'node:assert';
import { createRequire } from 'node:module';

// Two mutually unreachable CommonJS entries share one group chunk. Strict execution order must
// keep each entry behind its lazy `require_*` wrapper: requiring the silent entry runs no foreign
// top-level code, and both entries keep their own `module.exports`.
const require = createRequire(import.meta.url);

globalThis.__cjs_entry_group_isolation = [];
const silent = require('./dist/silent.js');
assert.deepStrictEqual(globalThis.__cjs_entry_group_isolation, []);
assert.deepStrictEqual(silent, { name: 'silent' });

const effect = require('./dist/effect.js');
assert.deepStrictEqual(globalThis.__cjs_entry_group_isolation, ['effect-entry']);
assert.deepStrictEqual(effect, { name: 'effect' });
