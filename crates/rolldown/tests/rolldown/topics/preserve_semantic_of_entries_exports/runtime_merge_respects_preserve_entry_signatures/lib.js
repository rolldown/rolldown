// Entry chunk. Its declared signature is exactly `{ foo, bar }`.
import assert from 'node:assert';

assert.strictEqual('lib runtime helper', 'lib runtime helper');

export const foo = 'foo_value';
export const bar = 42;
