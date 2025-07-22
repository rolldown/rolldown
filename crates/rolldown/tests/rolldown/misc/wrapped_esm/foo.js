export var a, [b] = [], [c = 1] = [];
export var d, {e} = {}, {f: g = 1} = {};
export var {h: [x, { y }], i: { z }, j: { k } = {k: null}} = {h: [0, {y: null}], i: {}};
export function foo() { }
export class bar { }
export default class baz { }

export { 
  a1,
  a2,
  a3,
  destructuring,
  index
}

if (globalVar) {
  var a1 = 1000;
}
switch (globalVar) {
  case true:
    var a2 = 'baz';
    break;
  default:
}
for (var index = 0; index < 10; index++) {
  var a3 = 1000;
}

var { destructuring } = {destructuring: 'destructuring'};
