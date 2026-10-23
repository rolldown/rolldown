import assert from 'node:assert/strict';
import { mainValue, loadDynamic } from './dist/main.js';

assert.equal(mainValue, 1);
assert.equal((await loadDynamic()).dynamicValue, 1);
