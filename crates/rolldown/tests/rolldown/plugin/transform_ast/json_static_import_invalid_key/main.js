import assert from 'node:assert/strict';
import value, {
  '😈' as devil,
  'property-name' as propertyName,
  'for' as reserved,
  normal,
} from './data.json';

assert.equal(globalThis.jsonStaticImportRan, true);
assert.equal(devil, 1);
assert.equal(propertyName, 2);
assert.equal(reserved, 3);
assert.equal(normal, 4);
assert.deepEqual(value, { '😈': 1, 'property-name': 2, for: 3, normal: 4 });
value.normal = 9;
assert.equal(normal, 4);

export {
  value as default,
  devil as '😈',
  propertyName as 'property-name',
  reserved as 'for',
  normal,
};
