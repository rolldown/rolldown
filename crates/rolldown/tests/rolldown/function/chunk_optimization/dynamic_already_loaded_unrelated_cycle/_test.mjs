import assert from 'node:assert';
import { value, loadLazy, loadSplash, loadBarrel } from './dist/main.js';

assert.strictEqual(value, 'sharedservice');
assert.strictEqual((await loadLazy()).lazy, 'lazyshared');
assert.strictEqual((await loadSplash()).splash, 'serviceslug');
assert.strictEqual(await loadBarrel(), 'service');
