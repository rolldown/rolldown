// Reads of function properties and of the prototype of a function.
function fn() {}
fn.a = 'DEAD_fn_property';
fn.a;
fn.prototype.a = 'DEAD_fn_prototype';
fn.prototype.a;

// `new` of a constructor that writes only to the new object.
class PureConstructor {
  constructor(value) {
    this.value = value;
  }
}
new PureConstructor('DEAD_class_constructor');
function Es5(value) {
  this.value = value;
}
new Es5('DEAD_es5_constructor');
class Fields {
  value = 'DEAD_field';
}
new Fields();

// Calls of static methods, also through `extends`.
class Statics {
  static method() {
    return 'DEAD_static_method';
  }
  static get getter() {
    return 'DEAD_static_getter';
  }
}
Statics.method();
Statics.getter;
class Child extends Statics {}
Child.method();

// A static block that writes only to the class.
class Block {
  static {
    this.value = 'DEAD_static_block';
  }
}

// A write to the prototype of a class that nothing reads.
class Proto {}
Proto.prototype.value = 'DEAD_class_prototype';

// Static fields fold branches, also through `extends`.
class Config {
  static debug = false;
}
class Derived extends Config {}
export function main() {
  if (Config.debug) console.log('DEAD_static_branch');
  if (Derived.debug) console.log('DEAD_inherited_branch');
  return 1;
}
