import { e as imported } from './value.js';
import * as ns from './value.js';

// Linked catches whose `var` lives in a function. The import prints at the top level and must not
// be captured by the catch parameter or its `var`.
export function inner() {
  const out = [];
  try {
    throw 1;
  } catch (e) {
    var e = 2;
    out.push(e, imported, ns.e);
  }
  out.push(e);
  return out;
}

export function nested() {
  const out = [];
  try {
    throw 1;
  } catch (e) {
    try {
      throw 2;
    } catch (e) {
      var e = 3;
      out.push(e, imported);
    }
    out.push(e, ns.e);
  }
  out.push(e);
  return out;
}

export function loops() {
  const out = [];
  try {
    throw 0;
  } catch (e) {
    for (var e of [1, 2]) out.push(e);
    for (var e in { k: 1 }) out.push(e);
    var { e } = { e: 3 };
    out.push(e);
    var [e] = [4];
    out.push(e, imported);
  }
  out.push(e);
  return out;
}

export function arrow() {
  return (() => {
    try {
      throw 0;
    } catch (e) {
      var e = 5;
      return [e, imported, (() => e)()];
    }
  })();
}
