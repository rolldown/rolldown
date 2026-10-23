globalThis.log = [];
const log = (msg) => globalThis.log.push(msg);
// The alias hides the callback from the `try` rule, so the walk of the callback checks it.
const attempt = (name, run) => {
  const call = run;
  try {
    call();
  } catch {
    log(name);
  }
};

// `new` of a method, an arrow function, an async function, or a generator throws.
const methods = { method() {} };
attempt('LIVE_new_method', () => {
  new methods.method();
});
const Arrow = () => {};
attempt('LIVE_new_arrow', () => {
  new Arrow();
});
const Async = async function () {};
attempt('LIVE_new_async', () => {
  new Async();
});
const Generator = function* () {};
attempt('LIVE_new_generator', () => {
  new Generator();
});
class WithMethods {
  method() {}
  static staticMethod() {}
}
attempt('LIVE_new_class_method', () => {
  new WithMethods.prototype.method();
});
attempt('LIVE_new_static_method', () => {
  new WithMethods.staticMethod();
});

// The default constructor of a derived class runs the parent constructor and the fields.
class Parent {
  constructor() {
    log('LIVE_parent_constructor');
  }
}
class Child extends Parent {}
new Child();
class Plain {}
class ChildField extends Plain {
  field = log('LIVE_derived_field');
}
new ChildField();
function Es5Parent() {
  log('LIVE_es5_parent');
}
class Es5Child extends Es5Parent {}
new Es5Child();
class GrandChild extends Child {}
new GrandChild();

// `super()` of `extends null` throws.
class NullParent extends null {}
attempt('LIVE_extends_null', () => {
  new NullParent();
});

// The fields go on the object that the parent constructor returns.
class FrozenParent {
  constructor() {
    return Object.freeze({});
  }
}
class FrozenChild extends FrozenParent {
  field = 1;
}
attempt('LIVE_frozen_parent', () => {
  new FrozenChild();
});
function FrozenEs5() {
  return Object.freeze({});
}
class FrozenEs5Child extends FrozenEs5 {
  field = 1;
}
attempt('LIVE_frozen_es5_parent', () => {
  new FrozenEs5Child();
});
class FrozenMiddle extends FrozenParent {}
class FrozenGrandChild extends FrozenMiddle {
  field = 1;
}
attempt('LIVE_frozen_grandparent', () => {
  new FrozenGrandChild();
});
class ReturnParent {
  constructor(target) {
    return target;
  }
}
class PrivateField extends ReturnParent {
  #mark = 1;
}
const fieldTarget = {};
new PrivateField(fieldTarget);
attempt('LIVE_private_field_twice', () => {
  new PrivateField(fieldTarget);
});
class PrivateMethod extends ReturnParent {
  #method() {}
}
const methodTarget = {};
new PrivateMethod(methodTarget);
attempt('LIVE_private_method_twice', () => {
  new PrivateMethod(methodTarget);
});

// A function expression runs its body.
const Logger = function () {
  log('LIVE_function_expression');
};
new Logger();
const holder = {
  Ctor: function () {
    log('LIVE_holder_constructor');
  },
};
new holder.Ctor();

export const done = true;
