import assert from 'node:assert';
import { manager } from './dist/main.js';

// manager.value must equal 'hello' (set in deep.js after two awaits).
// Without the fix, barrel's init calls init_middle() without await;
// the two-await delay means manager is still undefined when setup()
// runs, so this import will throw before we even reach these asserts.
assert.strictEqual(manager.value, 'hello');
assert.strictEqual(manager.ready, true);
