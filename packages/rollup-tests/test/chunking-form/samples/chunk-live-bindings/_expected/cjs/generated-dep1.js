'use strict';

function fn$2 () {
  console.log('lib fn');
}

function fn$1 () {
  fn$2();
  console.log(exports.text);
  exports.text$1 = 'dep1 fn after dep2';
}

exports.text$1 = 'dep1 fn';

function fn () {
  console.log(exports.text$1);
  exports.text = 'dep2 fn after dep1';
}

exports.text = 'dep2 fn';

exports.fn = fn;
exports.fn$1 = fn$1;
