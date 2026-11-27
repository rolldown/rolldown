import assert from 'node:assert/strict';

const actual = [];
if (process.argv.length > 0) {
  var [Object = () => {}] = [];
  actual.push(Object.name);
  var { Object = () => {} } = {};
  actual.push(Object.name);
  var { missing: Object = function () {} } = {};
  actual.push(Object.name);
}

assert.deepEqual(actual, ['Object', 'Object', 'Object']);
