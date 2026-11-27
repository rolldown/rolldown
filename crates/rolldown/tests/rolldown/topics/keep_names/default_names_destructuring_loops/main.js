import assert from 'node:assert/strict';

const actual = {};
var [Object = () => {}] = [];
actual.arrayDeclaration = Object.name;
var { Object = () => {} } = {};
actual.objectDeclaration = Object.name;
var { missing: Object = function () {} } = {};
actual.aliasedDeclaration = Object.name;

[Object = () => {}] = [];
actual.arrayAssignment = Object.name;
({ Object = () => {} } = {});
actual.objectAssignment = Object.name;
({ missing: Object = () => {} } = {});
actual.aliasedAssignment = Object.name;

for (var Object = () => {}; ;) {
  actual.forInitializer = Object.name;
  break;
}
for (var [, Object = () => {}] in { x: 0 }) actual.forIn = Object.name;
for (var { Object = () => {} } of [{}]) actual.forOf = Object.name;
for ([Object = () => {}] of [[]]) actual.forOfAssignment = Object.name;

assert.deepEqual(actual, {
  arrayDeclaration: 'Object',
  objectDeclaration: 'Object',
  aliasedDeclaration: 'Object',
  arrayAssignment: 'Object',
  objectAssignment: 'Object',
  aliasedAssignment: 'Object',
  forInitializer: 'Object',
  forIn: 'Object',
  forOf: 'Object',
  forOfAssignment: 'Object',
});
