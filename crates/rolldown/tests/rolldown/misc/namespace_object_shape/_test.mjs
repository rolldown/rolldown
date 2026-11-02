import assert from 'node:assert/strict';
import { load, names, namespace } from './dist/main.js';

// Each check below also passes on the native namespace objects of the source files.

// The dynamic import resolves to the same object as the static import.
const imported = await load();
assert.equal(imported, namespace);

// The namespace keeps the live binding.
assert.equal(namespace.value, 7);
namespace.increment();
assert.equal(namespace.value, 8);

// The namespace object has a null prototype, so it has no `Object.prototype` members.
assert.equal(Object.getPrototypeOf(namespace), null);
assert.equal('constructor' in namespace, false);
assert.equal('toString' in namespace, false);
assert.equal(namespace[Symbol.toStringTag], 'Module');
assert.equal(Object.prototype.toString.call(namespace), '[object Module]');
assert.deepEqual(Reflect.ownKeys(namespace), ['increment', 'value', Symbol.toStringTag]);

// The namespace object accepts no new properties. Module code is strict, so a failed assignment
// throws.
assert.equal(Object.isExtensible(namespace), false);
assert.equal(Reflect.set(namespace, 'extra', 9), false);
assert.equal(Reflect.defineProperty(namespace, 'extra', { value: 9 }), false);
assert.throws(() => {
  namespace.extra = 9;
}, TypeError);
assert.equal('extra' in namespace, false);

// The prototype stays null.
assert.equal(Reflect.setPrototypeOf(namespace, null), true);
assert.equal(Reflect.setPrototypeOf(namespace, {}), false);
assert.throws(() => Object.setPrototypeOf(namespace, {}), TypeError);

// Exports with the names of `Object.prototype` members are own properties.
assert.equal(Object.getPrototypeOf(names), null);
assert.deepEqual(Object.keys(names), ['__proto__', 'constructor', 'toString', 'valueOf']);
assert.equal(names.__proto__, 'proto export');
assert.equal(names.constructor(), 'constructor export');
assert.equal(names.toString(), 'toString export');
assert.equal(names.valueOf(), 'valueOf export');
// The conversion to a primitive calls the exported functions.
assert.equal(String(names), 'toString export');
assert.equal(names + '', 'valueOf export');
