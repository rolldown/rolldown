import assert from 'node:assert';
import { hasUseKey } from 'dep';

assert.strictEqual(useKey(), 'composable');
assert.strictEqual(hasUseKey(), false);
