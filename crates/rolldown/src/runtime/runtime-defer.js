// The helpers of `import defer`. The runtime module includes this file only when
// `experimental.importDefer` is on. Rolldown parses and scans the runtime module in each build, so
// a build without the option does not pay for these helpers.
// See internal-docs/runtime-helpers/implementation.md.

// `__deferNs` creates the deferred namespace object of an ES module (`import defer * as ns`).
// `init` evaluates the module and returns its namespace object `ns`. A static access such as
// `ns.foo` does not use the `Proxy`, so the module can evaluate before that. The `Proxy` evaluates
// the module for each operation with a string key other than `then`, and for `ownKeys`. A symbol
// key and `then` evaluate nothing, and `then` is always absent. The target has the same keys as
// the namespace, so that the `Proxy` invariants hold for `ownKeys` and `getOwnPropertyDescriptor`.
export var __deferNs = (init, ns) => {
  var target = Object.setPrototypeOf({}, null),
    seal = () => {
      for (var key of __getOwnPropNames(ns).sort()) {
        key === 'then' ||
          __defProp(target, key, { value: void 0, writable: true, enumerable: true });
      }
      Object.preventExtensions(target);
    },
    load = () => (init(), ns),
    deferred = (key) => typeof key === 'string' && key !== 'then',
    own = (key) => __hasOwnProp.call(target, key);
  __defProp(target, Symbol.toStringTag, { value: 'Deferred Module' });
  seal();
  return new Proxy(target, {
    get: (t, key) => (deferred(key) ? (load(), own(key) ? ns[key] : void 0) : t[key]),
    has: (t, key) => (deferred(key) ? (load(), own(key)) : key in t),
    getOwnPropertyDescriptor: (t, key) =>
      deferred(key)
        ? (load(),
          own(key)
            ? { value: ns[key], writable: true, enumerable: true, configurable: false }
            : void 0)
        : __getOwnPropDesc(t, key),
    defineProperty: (t, key, desc) =>
      deferred(key)
        ? (load(),
          own(key) &&
            !desc.configurable &&
            desc.enumerable !== false &&
            !('get' in desc || 'set' in desc) &&
            desc.writable !== false &&
            (!('value' in desc) || Object.is(desc.value, ns[key])))
        : own(key) && Reflect.defineProperty(t, key, desc),
    deleteProperty: (t, key) => (deferred(key) ? (load(), !own(key)) : Reflect.deleteProperty(t, key)),
    ownKeys: (t) => (load(), Reflect.ownKeys(t)),
    set: () => false,
    setPrototypeOf: (t, proto) => proto === null,
  });
};
