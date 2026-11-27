import assert from 'node:assert/strict';
import { foo as readFoo } from './read.cjs';
import { foo as callFoo } from './call.cjs';

assert.strictEqual(readFoo, 'foo');
assert.strictEqual(globalThis.version, '1.0.0');
assert.strictEqual(callFoo, 'foo');
assert.strictEqual(globalThis.initialized, true);
