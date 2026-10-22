globalThis.log = [];
const log = (msg) => globalThis.log.push(msg);

// A write to the prototype, and a write that shadows it.
const proto = { debug: false };
const child = { __proto__: proto };
proto.debug = true;
if (child.debug) log('LIVE_proto_write');
const proto2 = { debug: false };
const child2 = { __proto__: proto2 };
child2.debug = true;
if (child2.debug && !proto2.debug) log('LIVE_own_shadow');

// `Object.setPrototypeOf` changes the prototype.
const child3 = { __proto__: { debug: false } };
Object.setPrototypeOf(child3, { debug: true });
if (child3.debug) log('LIVE_set_prototype');

// A getter of the prototype runs, also after a `delete` of the own key.
const getterProto = {
  get x() {
    log('LIVE_proto_getter');
    return 1;
  },
};
const child4 = { __proto__: getterProto };
child4.x;
const child5 = { __proto__: getterProto, x: 0 };
delete child5.x;
child5.x;

// A computed `__proto__` key defines an own property.
const own = { ['__proto__']: { debug: true } };
if (own.__proto__.debug) log('LIVE_computed_proto_key');

// The prototype of a function.
function Ctor() {}
Ctor.prototype.debug = true;
const child6 = { __proto__: Ctor.prototype };
if (child6.debug) log('LIVE_fn_prototype');

// Code can add a key to `Object.prototype` and to `Function.prototype`.
Object.prototype.addedKey = true;
Function.prototype.addedKey = true;
const plain = {};
const withProto = { __proto__: { a: 1 } };
class Plain {}
if (plain.addedKey) log('LIVE_object_prototype_key');
if (withProto.addedKey) log('LIVE_object_prototype_chain');
if (Plain.addedKey) log('LIVE_function_prototype_key');
delete Object.prototype.addedKey;
delete Function.prototype.addedKey;

export const done = true;
