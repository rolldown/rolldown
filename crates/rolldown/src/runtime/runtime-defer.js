// The helpers of `import defer`. The runtime module includes this file only when
// `experimental.importDefer` is on. Rolldown parses and scans the runtime module in each build, so
// a build without the option does not pay for these helpers.
// See internal-docs/runtime-helpers/implementation.md.

// `__esmState` is the lazy ESM wrapper of a module that a deferred access (`import defer`) must
// observe. Like `__esmMin`, the init function evaluates the module one time and throws (or, for an
// async body, rejects with) a stored error again. It also keeps a record with the steps of spec
// `InnerModuleEvaluation`:
// - `s`: the state. `undefined` = not started, 1 = evaluating, 4 = evaluating-async (the sync part
//   returned, and the body waits), 2 = evaluated, 3 = error.
// - `e`: the error. `r`: the cycle root. `i` and `a`: the stack index and the ancestor index. The
//   stack position works as the DFS index, because only modules on the stack compare indexes.
//   `c`: the record of the importer that started the module. `b`: set when the body finished.
// - `p`: the result of the body (a promise for an async body). `t`: the module has its own
//   top-level await. `d`: returns the init functions of the tracked requested modules. `f`: the
//   init function. A helper that gets a hoisted `init_x` calls `f`, because the first call of the
//   hoisted function replaces it, and a second call of the old function creates a second record.
// `flags`: 1 = the body is async, 2 = the module has its own top-level await. An async body calls
// `__esmEnd(init_x)` at its end and `__esmFail(init_x, e)` for an error, because a promise cannot
// tell synchronously that the body finished. The body has no parameter: a parameter could shadow
// a top-level binding that the body reads.
// `init(__esmState)` returns the record and evaluates nothing. `init(callerInit)` evaluates the
// module for an importer that is also tracked. A call to a module that is evaluating then lowers
// the ancestor index of the importer, so that the importer stays on the stack until its cycle
// root finishes.
var __esmStack = [];
export var __esmState = (body, deps, flags, rec) => (
  (rec = { d: deps, t: flags & 2 }),
  (rec.f = (caller) => {
    if (caller === __esmState) return rec;
    var from = caller && caller(__esmState),
      x;
    if (rec.s) {
      if (rec.s === 1 && from && rec.a < from.a) from.a = rec.a;
      if ((x = __esmError(rec))) {
        if (flags & 1) return rec.p.catch(() => {}), Promise.reject(x[0]);
        throw x[0];
      }
      return rec.p;
    }
    rec.s = 1;
    rec.c = from;
    rec.i = rec.a = __esmStack.length;
    __esmStack.push(rec);
    if (flags & 1) {
      rec.p = body();
      // The sync part threw, and `__esmFail` popped the stack.
      if (rec.s === 3) return rec.p;
    } else {
      try {
        rec.p = body();
      } catch (e) {
        throw __esmFail(rec.f, e);
      }
      rec.b = 1;
    }
    if (rec.a === rec.i) {
      // The cycle root returned its sync part: pop its cycle.
      do ((x = __esmStack.pop()).r = rec), (x.s = x.b ? 2 : 4);
      while (x !== rec);
    } else if (from && rec.a < from.a) from.a = rec.a;
    return rec.p;
  })
);
// The end of an async body.
export var __esmEnd = (init, rec) => {
  rec = init(__esmState);
  rec.b = 1;
  if (rec.s === 4) rec.s = 2;
};
// An error of a body. In the sync part, each module on the stack above the module is in its cycle,
// and gets the error. Returns the error, for `throw`.
export var __esmFail = (init, e, rec, x) => {
  rec = init(__esmState);
  if (rec.s === 1) {
    do ((x = __esmStack.pop()).s = 3), (x.e = e);
    while (x !== rec);
  } else (rec.s = 3), (rec.e = e);
  return e;
};
// The stored error of a module, in a box: its own error, else the error of its cycle root.
export var __esmError = (m) => (m.s === 3 ? [m.e] : m.r && m.r.s === 3 ? [m.r.e] : void 0);
// The module finished: spec `IsModuleSCCEvaluated` (the cycle root decides), or it failed.
export var __esmComplete = (m) => m.s === 3 || (!!m.r && (m.r.s === 2 || m.r.s === 3));
// The module is in the evaluation walk of `from`: on the stack, or evaluating-async and an
// ancestor of `from` (Rolldown evaluates the import list of a module in sequence, across `await`).
export var __esmOnStack = (m, from) => {
  if (m.s === 1) return true;
  if (m.s === 4) for (; from; from = from.c) if (from === m) return true;
  return false;
};
// `__esmDepDone(init_y(init_x), init_y, init_x) || await init_y(init_x)`: an import of an
// async-bodied module y in a tracked module x waits only when y is not finished. When y failed,
// it throws the error synchronously. A module on the stack adds no wait (spec
// `InnerModuleEvaluation` step 14.2.3).
export var __esmDepDone = (p, dep, caller, m, x) => {
  m = dep(__esmState);
  if (__esmOnStack(m, caller && caller(__esmState))) return true;
  if ((x = __esmError(m))) {
    p && p.catch && p.catch(() => {});
    throw x[0];
  }
  return __esmComplete(m);
};
// `__esmGather(init_d, init_importer) || await __esmGather()`: the preparation of a deferred
// import of d whose closure has a module with top-level await (spec
// `GatherAsynchronousTransitiveDependencies`). The first call walks the records from d. It returns
// true when nothing is pending. Else it keeps the lists, and the second call returns a promise
// that waits for them: first each module with top-level await (an error rejects the importer),
// then each started module that waits (its error waits for the access).
export var __esmGather = (d, importer) => {
  if (!d) return (d = __esmStack.w), (__esmStack.w = 0), d();
  var from = importer && importer(__esmState),
    seen = new Set(),
    g = [],
    started = [],
    walk = (m) => {
      if (seen.has(m)) return;
      seen.add(m);
      if (__esmOnStack(m, from) || __esmComplete(m)) return;
      if (m.t) return g.push(m);
      if (m.s === 4) started.push(m);
      for (var dep of m.d()) walk(dep(__esmState));
    };
  walk(d(__esmState));
  if (!g.length && !started.length) return true;
  __esmStack.w = async () => {
    for (var m of g) if (m.s !== 2) await m.f(importer);
    for (m of started) await Promise.resolve(m.f()).then(
      () => {},
      () => {},
    );
  };
  return false;
};
// `__esmDefer` is a deferred access to a module with a `__esmState` record (spec
// `EnsureDeferredNamespaceEvaluation`). It throws a `TypeError`, and evaluates nothing, when the
// module or a module that it requests is evaluating (spec `ReadyForSyncExecution`). Else it
// evaluates the module synchronously, or throws its stored error.
export var __esmDefer = (init) => {
  var m = init(__esmState),
    seen = new Set(),
    ready = (m) =>
      seen.has(m) ||
      (seen.add(m),
      __esmComplete(m) || (m.s !== 1 && m.s !== 4 && !m.t && m.d().every((f) => ready(f(__esmState))))),
    p,
    x;
  if (!__esmComplete(m)) {
    if (!ready(m)) throw new TypeError('A deferred module or a module that it imports is evaluating');
    p = m.f();
  }
  if ((x = __esmError(m))) {
    (p || m.p) && (p || m.p).catch && (p || m.p).catch(() => {});
    throw x[0];
  }
  if (!__esmComplete(m)) throw new TypeError('A deferred module did not finish synchronously');
};
// `__deferNs` creates the deferred namespace object of a module (`import defer * as ns`).
// `init` evaluates the module and returns its namespace object. `ns` is that object when its keys
// are known before the evaluation. Without `ns`, the keys come from the first evaluation (a module
// with `export *` from CommonJS). The `Proxy` evaluates the module for each operation with a
// string key other than `then`, and for `ownKeys`. A symbol key and `then` evaluate nothing, and
// `then` is always absent. The target has the same keys as the namespace, so that the `Proxy`
// invariants hold for `ownKeys` and `getOwnPropertyDescriptor`.
export var __deferNs = (init, ns) => {
  var target = Object.setPrototypeOf({}, null),
    seal = () => {
      for (var key of __getOwnPropNames(ns).sort()) {
        key === 'then' ||
          __defProp(target, key, { value: void 0, writable: true, enumerable: true });
      }
      Object.preventExtensions(target);
    },
    load = () => (ns ? init() : ((ns = init()), seal()), ns),
    deferred = (key) => typeof key === 'string' && key !== 'then',
    own = (key) => __hasOwnProp.call(target, key);
  __defProp(target, Symbol.toStringTag, { value: 'Deferred Module' });
  ns && seal();
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
        : Reflect.defineProperty(t, key, desc),
    deleteProperty: (t, key) => (deferred(key) ? (load(), !own(key)) : Reflect.deleteProperty(t, key)),
    ownKeys: (t) => (load(), Reflect.ownKeys(t)),
    set: () => false,
  });
};
// `__toESM` with one result for each exports object and interop mode. A module of a deferred
// closure (`import defer`) loads an external lazily in CommonJS output, and the chunk can also
// load it eagerly. All these loads must give one namespace object.
// See internal-docs/runtime-helpers/implementation.md.
var __toESMCache = [];
export var __toESMCached = (mod, isNodeMode, cache, ns) => {
  if (mod !== Object(mod)) return __toESM(mod, isNodeMode);
  cache = __toESMCache[isNodeMode ? 1 : 0] || (__toESMCache[isNodeMode ? 1 : 0] = new WeakMap());
  if (!(ns = cache.get(mod))) cache.set(mod, (ns = __toESM(mod, isNodeMode)));
  return ns;
};
