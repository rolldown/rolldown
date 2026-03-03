import { URL as NodeURL, fn as otherFn } from './url.js';

// Case 1: Class expression in return statement (deconflicted)
export function createURL() {
  return class URL extends NodeURL {};
}

// Case 2: Function expression in return statement (deconflicted)
export function createFn() {
  return function fn() {
    return 1;
  };
}

// Case 3-5: Non-deconflicted inner scope — should NOT get __name
export function wrapper1() {
  function helper() {
    return 42;
  }
  return helper;
}

export function wrapper2() {
  class MyClass {}
  return MyClass;
}

export function wrapper3() {
  var myFn = function () {};
  var myArrow = () => {};
  var myClass = class {};
  return { myFn, myArrow, myClass };
}
