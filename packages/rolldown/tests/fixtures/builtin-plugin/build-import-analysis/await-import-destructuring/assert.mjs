import assert from 'node:assert';
import { foo, bar } from './dist/main';

assert.strictEqual(foo, 100);
assert.strictEqual(bar, 200);
