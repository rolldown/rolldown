import assert from 'node:assert';
import { run } from './dist/main.js';

// `import()` of the user entry `data.json` gives the values of the JSON module, although no code
// evaluates the module before it.
const data = [
  ['a', 1],
  ['b', 'x'],
  ['default', { a: 1, b: 'x' }],
];
assert.deepStrictEqual(await run(), {
  eager: data,
  deferred: data,
  sameEager: true,
  distinctDeferred: true,
});
