import { outer as value } from './value.js';

export function run() {
  const values = [];
  try {
    throw 1;
  } catch (outer) {
    var outer = 2;
    values.push(outer, value);
    {
      let outer = 3;
      values.push(outer);
    }
  }
  values.push(outer);
  return values;
}

export function iterate() {
  const values = [];
  try {
    throw 0;
  } catch (outer) {
    for (var outer of [4, 5]) values.push(outer);
    var { outer } = { outer: 6 };
    values.push(outer);
    var [outer] = [7];
    values.push(outer);
    for (var outer in { key: 1 }) values.push(outer);
    values.push(value);
  }
  values.push(outer);
  return values;
}

export function nested() {
  const values = [];
  try {
    throw 1;
  } catch (outer) {
    values.push(outer);
    try {
      throw 2;
    } catch (outer) {
      var outer = 3;
      values.push(outer, value);
    }
    values.push(outer);
  }
  values.push(outer);
  return values;
}
