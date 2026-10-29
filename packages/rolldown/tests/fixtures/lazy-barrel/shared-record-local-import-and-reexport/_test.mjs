import assert from 'node:assert';
// Without the fix, lazy barrel drops `store.js`, so the emitted `setup`
// references an unbound `store` and importing the bundle throws `ReferenceError`.
import { result } from './dist/entry.js';

assert.deepStrictEqual(result, ['setup-ran', 'helper']);
