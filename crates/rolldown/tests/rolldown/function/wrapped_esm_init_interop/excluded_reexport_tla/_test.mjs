import assert from 'node:assert';
import run from './dist/main.js';

globalThis.__log = [];
delete globalThis.__ready;

// Runtime regression check: the carrier observes the TLA side effect before it continues.
assert.strictEqual(await run(), 'usedmarker');
assert.deepStrictEqual(
  globalThis.__log,
  ['LEAF:before', 'LEAF:after', 'CARRIER:ready', 'BARREL:used:marker'],
  'an excluded re-export awaits its TLA init before the carrier continues',
);
