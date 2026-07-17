import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import value, {
  '😈' as devil,
  'property-name' as propertyName,
  'for' as reserved,
  normal,
} from './dist/main.js';

const preservedJson = await import('./dist/data.js');

assert.equal(globalThis.jsonStaticImportRan, true);
assert.equal(devil, 1);
assert.equal(propertyName, 2);
assert.equal(reserved, 3);
assert.equal(normal, 4);
assert.deepEqual(value, { '😈': 1, 'property-name': 2, for: 3, normal: 9 });
assert.equal(preservedJson['😈'], 1);
assert.equal(preservedJson['property-name'], 2);
assert.equal(preservedJson.for, 3);
assert.equal(preservedJson.normal, 4);
assert.equal(preservedJson.default.normal, 9);

const mainSource = await readFile(new URL('./dist/main.js', import.meta.url), 'utf8');
const dataSource = await readFile(new URL('./dist/data.js', import.meta.url), 'utf8');
const output = `${mainSource}\n${dataSource}`;

assert.match(output, /\[["']😈["']\]/);
assert.match(output, /\[["']property-name["']\]/);
assert.ok(!output.includes('.😈'));
assert.ok(!output.includes('.property-name'));
