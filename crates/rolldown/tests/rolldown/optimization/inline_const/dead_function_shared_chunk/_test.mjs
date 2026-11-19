import assert from 'node:assert/strict';
import { readdirSync } from 'node:fs';
import { a, forms } from './dist/a.js';
import { b } from './dist/b.js';

if (!['no-treeshake', 'no-inline'].includes(globalThis.__configName)) {
  assert.deepEqual(
    readdirSync(new URL('./dist/', import.meta.url))
      .filter((file) => file.endsWith('.js'))
      .sort(),
    ['a.js', 'b.js'],
  );
}
assert.equal(a('a'), 'a');
assert.equal(forms(), 'production');
assert.throws(() => a(''), { message: 'https://example.com/oops' });
const warn = console.warn;
const messages = [];
try {
  console.warn = (...args) => messages.push(args);
  assert.equal(b(''), '');
} finally {
  console.warn = warn;
}
assert.deepEqual(messages, [['https://example.com/careful']]);
