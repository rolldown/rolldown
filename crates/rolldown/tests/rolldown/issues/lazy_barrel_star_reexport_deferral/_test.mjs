import assert from 'node:assert';
// Star re-exports resolved via on-demand probing must still bind correctly.
import { result } from './dist/main.js';
assert.strictEqual(result, 'a-value|c-value');
