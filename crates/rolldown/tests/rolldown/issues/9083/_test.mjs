import assert from 'node:assert';
import { manager } from './dist/main.js';

// Without the fix, barrel init calls init_middle() without awaiting it, so the
// entry evaluates before `manager` is initialized and this import fails.
assert.strictEqual(manager.value, 'hello');
assert.strictEqual(manager.ready, true);
