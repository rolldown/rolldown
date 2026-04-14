import assert from 'node:assert';
import { manager, setup } from './dist/main.js';

setup();

assert.strictEqual(manager.value, 'hello');
assert.strictEqual(manager.ready, true);
