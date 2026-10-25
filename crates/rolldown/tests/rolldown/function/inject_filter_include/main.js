import assert from 'node:assert';
import { hasUseKey } from 'dep';
import { appKey } from './src/app.js';

assert.strictEqual(appKey(), 'composable');
assert.strictEqual(hasUseKey(), false);
assert.strictEqual(typeof useKey, 'undefined');
