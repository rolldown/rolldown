'use strict';

var dep2 = require('./generated-dep2.js');

function fn$1 () {
  console.log('lib1 fn');
}

function fn () {
  fn$1();
  console.log('dep3 fn');
}

class Main2 {
  constructor () {
    fn();
    dep2.fn();
  }
}

module.exports = Main2;
//# sourceMappingURL=main2.js.map
