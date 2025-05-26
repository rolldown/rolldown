export var a, [b] = [], [c = 1] = [];
export var d, {e} = {}, {f: g = 1} = {};
export var {h: [x, { y }], i: { z }, j: { k } = {k: null}} = {h: [0, {y: null}], i: {}};
export function foo() { }
export class bar { }
export default class baz { }

export { }
