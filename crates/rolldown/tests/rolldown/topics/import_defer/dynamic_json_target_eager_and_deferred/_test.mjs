import assert from 'node:assert';
import { run } from './dist/main.js';

// `import()` gives the namespace of the JSON module, with no export that the chunk adds for
// `import.defer()`. Each `import()` gives the same object, and `import.defer()` gives another one.
const first = [
  ['a', 1],
  ['b', 'x'],
  ['default', { a: 1, b: 'x' }],
];
const second = [
  ['c', [2]],
  ['default', { c: [2] }],
];
const local = [
  ['d', true],
  ['default', { d: true }],
];
assert.deepStrictEqual(await run(), {
  first: { eager: first, deferred: first, sameEager: true, distinctDeferred: true },
  second: { eager: second, deferred: second, sameEager: true, distinctDeferred: true },
  local: { eager: local, deferred: local, sameEager: true, distinctDeferred: true, static: true },
});
