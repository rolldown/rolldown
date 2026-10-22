globalThis.log = [];

// A getter of an object literal.
const a = { v: false };
const getter = {
  get g() {
    return a;
  },
};
getter.g.v = true;
if (a.v) globalThis.log.push('LIVE_object_getter');

// A getter on the prototype of an object literal.
const b = { v: false };
const base = {
  get g() {
    return b;
  },
};
const child = { __proto__: base };
child.g.v = true;
if (b.v) globalThis.log.push('LIVE_proto_getter');

// A static getter of a class, also through `extends`.
const c = { v: false };
class Static {
  static get g() {
    return c;
  }
}
Static.g.v = true;
if (c.v) globalThis.log.push('LIVE_static_getter');
const d = { v: false };
class Parent {
  static get g() {
    return d;
  }
}
class Child extends Parent {}
Child.g.v = true;
if (d.v) globalThis.log.push('LIVE_inherited_static_getter');

// A getter of a class instance.
const e = { v: false };
class Instance {
  get g() {
    return e;
  }
}
new Instance().g.v = true;
if (e.v) globalThis.log.push('LIVE_instance_getter');

// A getter that returns a property of an other object, and a getter that returns a literal.
const f = { v: false };
const holder = { f };
const nested = {
  get g() {
    return holder.f;
  },
};
nested.g.v = true;
if (f.v) globalThis.log.push('LIVE_nested_getter');
const h = { v: false };
const literal = {
  get g() {
    return { h };
  },
};
literal.g.h.v = true;
if (h.v) globalThis.log.push('LIVE_getter_literal');

// Calls of a function, a method, and an arrow function that return the object.
const i = { v: false };
function fn() {
  return i;
}
fn().v = true;
if (i.v) globalThis.log.push('LIVE_function_return');
const j = { v: false };
const methods = {
  m() {
    return j;
  },
};
methods.m().v = true;
if (j.v) globalThis.log.push('LIVE_method_return');
const k = { v: false };
const arrow = () => k;
arrow().v = true;
if (k.v) globalThis.log.push('LIVE_arrow_return');

export const done = true;
