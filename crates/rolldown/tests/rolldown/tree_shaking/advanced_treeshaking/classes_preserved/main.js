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

// A static block writes through a static setter.
class StaticSetter {
  static set v(x) {
    log('LIVE_static_setter');
  }
  static {
    this.v = 1;
  }
}

// A static block writes through a setter of the parent.
class ParentSetter {
  static set v(x) {
    log('LIVE_parent_setter');
  }
}
class ChildOfSetter extends ParentSetter {
  static {
    this.v = 1;
  }
}

// The constructor writes through a setter of the prototype.
class InstanceSetter {
  set v(x) {
    log('LIVE_instance_setter');
  }
  constructor() {
    this.v = 1;
  }
}
new InstanceSetter();

// A new prototype brings a setter.
function Es5Replaced() {
  this.v = 1;
}
Es5Replaced.prototype = {
  set v(x) {
    log('LIVE_es5_setter');
  },
};
new Es5Replaced();

// Unknown code defines a setter on the prototype of a function.
function Es5Defined() {
  this.v = 1;
}
Object.defineProperty(Es5Defined.prototype, 'v', {
  set(x) {
    log('LIVE_es5_define_setter');
  },
});
new Es5Defined();

// Unknown code defines a setter on the prototype of a class.
class ClassDefined {
  constructor() {
    this.v = 1;
  }
}
Object.defineProperty(ClassDefined.prototype, 'v', {
  set(x) {
    log('LIVE_class_define_setter');
  },
});
new ClassDefined();

// Unknown code changes the prototype of the prototype.
class ProtoChanged {
  constructor() {
    this.v = 1;
  }
}
Object.setPrototypeOf(ProtoChanged.prototype, {
  set v(x) {
    log('LIVE_set_prototype_setter');
  },
});
new ProtoChanged();

// A static getter runs on a read.
class StaticGetter {
  static get v() {
    log('LIVE_static_getter');
    return 1;
  }
}
StaticGetter.v;

// A static getter of the parent runs on a read.
class ParentGetter {
  static get v() {
    log('LIVE_inherited_getter');
    return 1;
  }
}
class ChildOfGetter extends ParentGetter {}
ChildOfGetter.v;

// Unknown code defines a getter on a function.
function FnDefined() {}
Object.defineProperty(FnDefined, 'a', {
  get() {
    log('LIVE_fn_getter');
  },
});
FnDefined.a;

// A write to `__proto__` brings a getter.
function FnProto() {}
FnProto.__proto__ = {
  get a() {
    log('LIVE_fn_proto_getter');
  },
};
FnProto.a;

// A static method writes a static field through the class name.
class ByName {
  static flag = false;
  static enable() {
    ByName.flag = true;
  }
}
ByName.enable();
if (ByName.flag) log('LIVE_static_by_name');

// A static method writes a static field through `this`.
class ByThis {
  static flag = false;
  static enable() {
    this.flag = true;
  }
}
ByThis.enable();
if (ByThis.flag) log('LIVE_static_by_this');

// A static arrow function writes a static field through `this`.
class ByArrow {
  static flag = false;
  static enable = () => {
    this.flag = true;
  };
}
ByArrow.enable();
if (ByArrow.flag) log('LIVE_static_by_arrow');

// A static block writes a static field.
class ByBlock {
  static flag = false;
  static {
    this.flag = true;
  }
}
if (ByBlock.flag) log('LIVE_static_by_block');

// The prototype leads back to the class.
class ByConstructor {
  static flag = false;
  enable() {
    this.constructor.flag = true;
  }
}
ByConstructor.prototype.enable();
if (ByConstructor.flag) log('LIVE_by_constructor');

// A class expression writes a static field through its own name.
const ByInnerName = class Inner {
  static flag = false;
  static enable() {
    Inner.flag = true;
  }
};
ByInnerName.enable();
if (ByInnerName.flag) log('LIVE_by_inner_name');

// A write and a deeper write to static fields.
class Written {
  static flag = false;
  static cfg = { on: false };
}
Written.flag = true;
Written.cfg.on = true;
if (Written.flag) log('LIVE_static_write');
if (Written.cfg.on) log('LIVE_static_deep_write');

// A write to a static field of the parent.
class Parent {
  static flag = false;
}
class Child extends Parent {}
Parent.flag = true;
if (Child.flag) log('LIVE_parent_write');

// A `delete` of a static field.
class Deleted {
  static flag = true;
}
delete Deleted.flag;
if (!Deleted.flag) log('LIVE_static_delete');

// `new` passes arguments to the constructor.
function Es5Param(flag) {
  if (flag) log('LIVE_es5_param');
}
Es5Param(false);
new Es5Param(true);
class ClassParam {
  constructor(flag) {
    if (flag) log('LIVE_class_param');
  }
}
new ClassParam(true);
function SpreadParam(flag) {
  if (flag) log('LIVE_spread_param');
}
new SpreadParam(...[true]);

// Fields and constructors with effects.
class FieldEffect {
  x = log('LIVE_field');
}
new FieldEffect();
class FieldGetter {
  get g() {
    log('LIVE_field_getter');
    return 1;
  }
  x = this.g;
}
new FieldGetter();
class ParentCtor {
  constructor() {
    log('LIVE_parent_ctor');
  }
}
class Derived extends ParentCtor {}
new Derived();
class ReturnEffect {
  constructor() {
    return log('LIVE_ctor_return');
  }
}
new ReturnEffect();

// A static setter that reads `this` runs during the definition.
class SetterThis {
  static set v(x) {
    this.w = x;
    log('LIVE_setter_this');
  }
  static {
    this.v = 1;
  }
}

// Code that throws.
class GetterOnly {
  get v() {
    return 1;
  }
  constructor() {
    this.v = 2;
  }
}
attempt('LIVE_getter_only', () => {
  new GetterOnly();
});
class Caller {}
attempt('LIVE_class_caller', () => {
  Caller.caller;
});
function FnCaller() {}
attempt('LIVE_fn_caller', () => {
  FnCaller.caller;
});
class Named {}
attempt('LIVE_class_name', () => {
  Named.name = 'x';
});
async function AsyncFn() {}
attempt('LIVE_new_async', () => {
  new AsyncFn();
});
const Arrow = () => {};
attempt('LIVE_new_arrow', () => {
  new Arrow();
});
class NotCallable {}
attempt('LIVE_call_class', () => {
  NotCallable();
});
class Frozen {}
Object.freeze(Frozen.prototype);
attempt('LIVE_frozen_prototype', () => {
  Frozen.prototype.x = 1;
});

export const done = true;
