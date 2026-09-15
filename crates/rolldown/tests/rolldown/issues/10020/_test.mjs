import assert from 'node:assert';
// #10020: importing the entry must succeed. Before the fix `dist/data.js` emitted
// `export { Abel, aBeeZee, ... }` for the JSON keys, whose bindings were inlined away,
// so this import threw `SyntaxError: Export 'Abel' is not defined in module` at ESM
// link time. Loading it here proves linking succeeds and the default import still works.
import { get } from './dist/index.js';

assert.deepStrictEqual(get('aBeeZee'), [0, 920, -262, 0, 1000, 481]);
assert.deepStrictEqual(get('Abel'), [0, 750, -250, 0, 1000, 500]);
