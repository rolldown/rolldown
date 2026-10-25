import assert from 'node:assert';
import { hasUseKey } from './vendor/dep.js';

assert.strictEqual(useKey(), 'composable');
assert.strictEqual(hasUseKey(), false);
