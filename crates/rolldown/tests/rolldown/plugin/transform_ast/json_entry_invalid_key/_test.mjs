import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import value, {
  '😈' as devil,
  'property-name' as propertyName,
  'for' as reserved,
  normal,
  "single'quote" as singleQuote,
  'line\nbreak' as lineBreak,
  'back\\slash' as backslash,
  '__proto__' as proto,
} from './dist/data.js';

assert.equal(globalThis.jsonEntrySideEffectRan, true);
assert.equal(devil, 1);
assert.equal(propertyName, 2);
assert.equal(reserved, 3);
assert.equal(normal, 4);
assert.equal(singleQuote, 5);
assert.equal(lineBreak, 6);
assert.equal(backslash, 7);
assert.deepEqual(proto, { safe: true });
assert.equal(Object.prototype.hasOwnProperty.call(value, '__proto__'), true);
assert.equal(Object.getPrototypeOf(value), Object.prototype);
assert.deepEqual(value, {
  '😈': 1,
  'property-name': 2,
  for: 3,
  normal: 4,
  "single'quote": 5,
  'line\nbreak': 6,
  'back\\slash': 7,
  ['__proto__']: { safe: true },
});

const output = await readFile(new URL('./dist/data.js', import.meta.url), 'utf8');
assert.match(output, /\[["']😈["']\]/);
assert.match(output, /\[["']property-name["']\]/);
assert.ok(!output.includes('.😈'));
assert.ok(!output.includes('.property-name'));
assert.ok(!output.includes("'single'quote'"));
assert.ok(!output.includes('"line\nbreak"'));
