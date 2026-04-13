import assert from 'node:assert';
import { manager } from './dist/main.js';

// manager.ready should be true because setup() was called in main.js
// This fails if barrel's init does not await init_middle() (transitive TLA bug)
assert.strictEqual(manager.ready, true);
