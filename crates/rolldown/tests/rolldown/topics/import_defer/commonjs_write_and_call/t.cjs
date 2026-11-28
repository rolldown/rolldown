const log = require('./log.cjs');

log.push('t');
exports.self = function () {
  return this;
};
exports.tag = function () {
  return this;
};
exports.obj = { value: 1 };
exports.foo = 'foo';
