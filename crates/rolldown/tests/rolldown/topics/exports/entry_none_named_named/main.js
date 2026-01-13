import assert from 'node:assert';
import { foo, bar } from './lib.js';

// Access named exports from pure ESM entry chunk with Named OutputExports
assert.strictEqual(foo, 'foo_value');
assert.strictEqual(bar, 42);

export { foo, bar };
