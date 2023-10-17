define(['exports', './generated-lib1'], (function (exports, lib1) { 'use strict';

  function fn$2 () {
    console.log('lib2 fn');
  }

  function fn$1 () {
    fn$2();
    console.log('dep2 fn');
  }

  function fn () {
    lib1.fn();
    console.log('dep3 fn');
  }

  exports.fn = fn$1;
  exports.fn$1 = fn;

}));
