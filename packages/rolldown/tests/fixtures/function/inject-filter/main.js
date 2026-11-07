import assert from 'node:assert';
import { hasUseKey } from './vendor/dep.js';
import { layerKey } from './vendor/layer/index.js';
import { virtualHasUseKey } from 'virtual:dep';

assert.strictEqual(useKey(), 'composable');
assert.strictEqual(hasUseKey(), false);
assert.strictEqual(layerKey(), 'composable');
// like rollup's `createFilter`, ids with a null byte never match a filter
assert.strictEqual(virtualHasUseKey(), false);
