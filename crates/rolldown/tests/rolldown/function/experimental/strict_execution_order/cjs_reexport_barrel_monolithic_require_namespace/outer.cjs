const ns = require('./wrapper.js');

module.exports = {
  cloned: ns.cloneDeep({ value: 1 }),
  cn: ns.cn('x'),
  keys: Object.keys(ns).sort(),
};
