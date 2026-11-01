export var __create = Object.create;
export var __moduleSyntaxError = SyntaxError;
export var __moduleTypeError = TypeError;
var Promise = globalThis.Promise;
export var __moduleResolutionErrors = new WeakSet();
export var __moduleLinkErrors = new WeakSet();
export var __moduleLinkError = message => {
  var error = new __moduleSyntaxError(message);
  __moduleLinkErrors.add(error);
  return error;
};
export var __rethrowModuleLinkError = error => {
  if (__moduleLinkErrors.has(error)) throw new __moduleSyntaxError(error.message);
  throw error;
};
var __moduleURL = globalThis.URL;
export var __importNamespace = async (initialize, namespace) => {
  await 0;
  await initialize();
  return namespace;
};
export var __computedImport = (specifier, options, candidates, base) => {
  try {
    // ToString follows both authored arguments, but precedes attribute processing.
    specifier = `${specifier}`;
  } catch (error) {
    return (async () => { throw error; })();
  }
  try {
    var attributes = { __proto__: null };
    for (var [key, value] of __readImportAttributes(options)) attributes[key] = value;
    options = { with: attributes };
    var absolute, origin;
    for (var [request, load, self] of candidates) {
      if (specifier === request) return load(options);
      if (self && specifier === base()) return load(options);
      if (!request.startsWith("./") && !request.startsWith("../")) continue;
      if (origin === void 0) origin = base();
      if (absolute === void 0) absolute = new __moduleURL(specifier, origin).href;
      if (absolute === new __moduleURL(request, origin).href) return load(options);
    }
    throw new __moduleTypeError("Unknown captured dynamic import: " + specifier + " (declare runtime-computed targets as native:dynamic-import-contexts candidates)");
  } catch (error) {
    return (async () => { throw error; })();
  }
};
var __readImportAttributes = options => {
    var entries = [];
    if (options !== void 0) {
      if (options === null || typeof options !== "object" && typeof options !== "function")
        throw new __moduleTypeError("Import options must be an object");
      var attributes = options.with;
      if (attributes !== void 0) {
        if (attributes === null || typeof attributes !== "object" && typeof attributes !== "function")
          throw new __moduleTypeError("Import attributes must be an object");
        for (var key of __getOwnPropNames(attributes)) {
          if (!__getOwnPropDesc(attributes, key)?.enumerable) continue;
          entries[entries.length] = [key, attributes[key]];
        }
        for (var entry of entries)
          if (typeof entry[1] !== "string")
            throw new __moduleTypeError("Import attribute values must be strings");
      }
    }
    return entries;
};
export var __withImportOptions = (options, load, type = false) => {
  try {
    var entries = __readImportAttributes(options);
    for (var [key, value] of entries)
      if (key !== "type" || value !== (type === true ? "json" : type))
        throw new __moduleTypeError("Unsupported import attribute: " + key);
    return load();
  } catch (error) {
    return (async () => { throw error; })();
  }
};
export var __rejectModuleResolution = async (message, options) => {
  if (options !== void 0) {
    if (options === null || typeof options !== "object" && typeof options !== "function")
      throw new __moduleTypeError("Import options must be an object");
    var attributes = options.with;
    if (attributes !== void 0) {
      if (attributes === null || typeof attributes !== "object" && typeof attributes !== "function")
        throw new __moduleTypeError("Import attributes must be an object");
      for (var key of __getOwnPropNames(attributes))
        if (__getOwnPropDesc(attributes, key)?.enumerable && typeof attributes[key] !== "string")
          throw new __moduleTypeError("Import attribute values must be strings");
    }
  }
  var error = new __moduleTypeError(message);
  __moduleResolutionErrors.add(error);
  throw error;
};
export var __defProp = Object.defineProperty;
export var __name = (target, value) => __defProp(target, 'name', { value, configurable: true });
export var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
export var __getOwnPropNames = Object.getOwnPropertyNames;
export var __getProtoOf = Object.getPrototypeOf;
export var __hasOwnProp = Object.prototype.hasOwnProperty;
export var __checkTDZ = (initialized, name, readonly) => {
  if (!initialized) throw new ReferenceError("Cannot access '" + name + "' before initialization");
  if (readonly) throw new TypeError("Assignment to constant variable.");
};
export var __startAsync = (pending, result) => {
  if (result) {
    // A later synchronous sibling may throw before the parent reaches the join.
    result.catch(() => {});
    pending.push(result);
  }
};
export var __finishAsync = (pending, execute) =>
  pending.length ? Promise.all(pending).then(execute) : execute();
var __asyncModules = [];
export var __esmAsync = (fn, state = {}) => () => {
  let root = state;
  while (root.parent) root = root.parent;
  if (root.active) {
    // An active backedge joins the intervening modules to one evaluation cycle.
    for (let index = __asyncModules.indexOf(root) + 1; index < __asyncModules.length; index++) {
      __asyncModules[index].parent = root;
    }
  }
  if (state.started) {
    if (root.error) throw root.error[0];
    // Initial DFS edges use local completion; later imports await the cycle root.
    return root.active ? state.result : root.result;
  }
  state.started = state.active = true;
  __asyncModules.push(state);
  try {
    const execute = typeof fn === "function" ? fn : fn[__getOwnPropNames(fn)[0]];
    fn = 0;
    return state.result = execute();
  } catch (error) {
    state.error = [error];
    throw error;
  } finally {
    state.active = false;
    __asyncModules.pop();
  }
};
var __esmProbe = 0;
var __esmRun = root => {
  if (root.err) throw root.err[0];
  if (!root.started) {
    let stack = [], pending = [], evaluation = {}, order = 0;
    let start = node => {
      node.started = true;
      node.evaluation = evaluation;
      node.order = node.ancestor = order++;
      pending.push(node);
      stack.push({ node, idx: 0 });
    };
    let fail = error => {
      let err = [error];
      for (let node of pending) {
        node.err = err;
        node.evaluation = 0;
      }
      pending.length = 0;
      throw error;
    };
    start(root);
    while (stack.length) {
      let top = stack[stack.length - 1];
      let node = top.node;
      let deps = node.deps;
      if (deps && top.idx < deps.length) {
        let depFn = deps[top.idx++];
        let dep;
        __esmProbe = 1;
        try {
          dep = depFn();
        } catch (e) {
          __esmProbe = 0;
          fail(e);
        }
        __esmProbe = 0;
        if (!dep || !dep._esm) continue;
        if (dep.err) fail(dep.err[0]);
        if (!dep.started) {
          start(dep);
        } else if (dep.evaluation === evaluation) {
          node.ancestor = Math.min(node.ancestor, dep.order);
        }
        continue;
      }
      stack.pop();
      node.deps = 0;
      let fn = node.fn;
      if (fn && !node.err) {
        node.fn = 0;
        try {
          node.res = node.named ? (0, fn[__getOwnPropNames(fn)[0]])() : fn();
        } catch (e) {
          fail(e);
        }
      }
      if (node.ancestor === node.order) {
        let completed;
        do {
          completed = pending.pop();
          completed.evaluation = 0;
        } while (completed !== node);
      }
      if (stack.length) {
        let parent = stack[stack.length - 1].node;
        parent.ancestor = Math.min(parent.ancestor, node.ancestor);
      }
    }
  }
  return root.res;
};
export var __esm = (fn, deps) => __esmMin(fn, deps, true);
export var __esmMin = (fn, deps, named) => {
  let state = { fn, deps, named, started: false, res: void 0, err: void 0, _esm: 1 };
  return () => (__esmProbe ? ((__esmProbe = 0), state) : __esmRun(state));
};
export var __commonJS = (cb, mod) =>
  function () {
    return (
      mod || ((0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod, mod.exports), cb = null), mod.exports
    );
  };
export var __commonJSMin = (cb, mod) => () => (
  mod || (cb((mod = { exports: {} }).exports, mod, mod.exports), cb = null), mod.exports
);
// A namespace whose star exports reach a CommonJS module, keyed to the getters of its exports and
// its Proxy's target, which `__copyProps` extends with the names that the module gives at runtime.
var __starNamespaces = new WeakMap();
export var __exportAll = (all, no_symbols, fixed) => {
  let star = __starNamespaces.get(all);
  if (star) {
    // The module has run its star exports, after which the namespace takes no other names.
    Object.preventExtensions(star[1]);
    return all;
  }
  let target = __create(null);
  let names = __getOwnPropNames(all).sort();
  for (let name of names) {
    __defProp(target, name, { value: undefined, writable: true, enumerable: true });
  }
  if (!no_symbols) __defProp(target, Symbol.toStringTag, { value: 'Module' });
  if (fixed) Object.preventExtensions(target);
  let namespace = new Proxy(target, {
    get: (target, key) => __hasOwnProp.call(all, key) ? all[key]() : target[key],
    ownKeys: (target) => {
      let keys = fixed ? names : __getOwnPropNames(target).sort();
      return no_symbols ? keys : [...keys, Symbol.toStringTag];
    },
    getOwnPropertyDescriptor(target, key) {
      let descriptor = __getOwnPropDesc(target, key);
      if (descriptor && __hasOwnProp.call(all, key)) descriptor.value = all[key]();
      return descriptor;
    },
    set: () => false,
    defineProperty(target, key, descriptor) {
      if (!__hasOwnProp.call(all, key)) {
        return __hasOwnProp.call(target, key) && Reflect.defineProperty(target, key, descriptor);
      }
      let value = all[key]();
      return !descriptor.configurable && descriptor.enumerable !== false &&
        descriptor.writable !== false && !('get' in descriptor) && !('set' in descriptor) &&
        (!('value' in descriptor) || Object.is(descriptor.value, value));
    },
    // A namespace that star exports complete stays extensible until its module has run them,
    // `__exportAll(namespace)`, and refuses to stop before: a cycle that inspects it then still
    // finds the names that a later star adds.
    preventExtensions: (target) => !Object.isExtensible(target),
    setPrototypeOf: (_, prototype) => prototype === null,
  });
  if (!fixed) __starNamespaces.set(namespace, [all, target]);
  return namespace;
};
export var __copyProps = (to, from, except, desc) => {
  if ((from && typeof from === 'object') || typeof from === 'function') {
    let star = __starNamespaces.get(to);
    for (var keys = __getOwnPropNames(from), i = 0, n = keys.length, key; i < n; i++) {
      key = keys[i];
      if (key === except) continue;
      if (star) {
        // A namespace's descriptor of a name runs its getter, so look the name up among the getters.
        if (!__hasOwnProp.call(star[0], key) && Object.isExtensible(star[1])) {
          __defProp(star[0], key, { value: ((k) => from[k]).bind(null, key), enumerable: true });
          __defProp(star[1], key, { value: undefined, writable: true, enumerable: true });
        }
      } else if (!__hasOwnProp.call(to, key)) {
        __defProp(to, key, {
          get: ((k) => from[k]).bind(null, key),
          enumerable: __starNamespaces.has(from) || !(desc = __getOwnPropDesc(from, key)) || desc.enumerable,
        });
      }
    }
  }
  return to;
};
export var __reExport = (target, mod, secondTarget) => (
  __copyProps(target, mod, 'default'), secondTarget && __copyProps(secondTarget, mod, 'default')
);
var __dynamicNamespaces = new WeakMap();
export var __toESM = (mod, isNodeMode, key, target) => {
  let modes = key && __dynamicNamespaces.get(key);
  let mode = isNodeMode ? 1 : 0;
  if (modes && modes.has(mode)) return modes.get(mode);
  // Read names that `module.exports` answers but does not own, such as a Proxy's or an inherited
  // getter's, through to it with it as the receiver, as webpack's live `exports.name` access does.
  if (mod != null && (typeof mod == 'object' || typeof mod == 'function')) {
    let proto = __getProtoOf(mod);
    // Except a `then` it lacks: a namespace that reads one it would not otherwise have is a
    // thenable, so its `import()`, or an async function that returns it, never settles. The
    // filter's own target inherits what the upstream namespace would, and owns nothing a trap must
    // report.
    let hide = !__hasOwnProp.call(mod, 'then') && !(proto != null && 'then' in proto);
    target = __create(
      new Proxy(__create(proto), {
        get: (_, name) => (hide && name === 'then' ? undefined : Reflect.get(mod, name, mod)),
        has: (_, name) => !(hide && name === 'then') && Reflect.has(mod, name),
        // V8's `for…in` does not enumerate past a Proxy, so it reports the names the
        // upstream namespace would inherit, up to the same boundary.
        ownKeys: () => {
          let names = [];
          for (let name in __create(proto)) names.push(name);
          return names;
        },
        getOwnPropertyDescriptor: (_, name) => {
          for (let from = proto, desc; from != null; from = __getProtoOf(from))
            if ((desc = __getOwnPropDesc(from, name))) return { ...desc, configurable: true };
        },
        // Those traps hold only while the target stays empty and extensible.
        defineProperty: () => false,
        preventExtensions: () => false,
        setPrototypeOf: () => false,
      }),
    );
  } else target = mod != null ? __create(__getProtoOf(mod)) : {};
  __copyProps(
    // `__esModule` alone is not enough: the module must own a `default` (#10360).
    isNodeMode || !mod || !mod.__esModule || !__hasOwnProp.call(mod, 'default')
      ? __defProp(target, 'default', { value: mod, enumerable: true })
      : target,
    mod,
  );
  if (key) {
    if (!modes) __dynamicNamespaces.set(key, modes = new Map());
    modes.set(mode, target);
  }
  return target;
};
export var __toCommonJS = (mod) =>
  __hasOwnProp.call(mod, 'module.exports')
    ? mod['module.exports']
    : __copyProps(__defProp({}, '__esModule', { value: true }), mod);
// `__share*`: the registry behind `codeSplitting.experimentalInlineCommonChunks`. A chunk factory
// is registered under its record id by every file that carries it; the first registration wins,
// `__share_require` runs the winning factory once and hands every reader the same exports object.
// See internal-docs/inline-common-chunks/implementation.md.
export var __share_factories = /* @__PURE__ */ __create(null);
export var __share_records = /* @__PURE__ */ __create(null);
export var __share = (id, factory) => {
  if (!(id in __share_factories)) __share_factories[id] = factory;
};
export var __share_require = (id) => {
  var record = __share_records[id];
  if (record) {
    // A failed factory rethrows the same value on every read, including `undefined`; a factory
    // that is still running hands back its partially filled exports object.
    if (record.failed) throw record.error;
    return record.exports;
  }
  var factory = __share_factories[id];
  if (!factory) throw new Error('Shared chunk "' + id + '" was not registered before it was required.');
  record = __share_records[id] = { exports: {}, failed: false, error: void 0 };
  try {
    factory(record.exports);
  } catch (error) {
    record.failed = true;
    record.error = error;
    throw error;
  }
  return record.exports;
};
export var __share_export = (target, all) => {
  for (var name in all) __defProp(target, name, { get: all[name], enumerable: true });
};
export var __toBinaryNode = (base64) => new Uint8Array(Buffer.from(base64, 'base64'));
export var __toBinary = /* @__PURE__ */ (() => {
  var table = new Uint8Array(128);
  for (var i = 0; i < 64; i++) {
    table[i < 26 ? i + 65 : i < 52 ? i + 71 : i < 62 ? i - 4 : i * 4 - 205] = i;
  }
  return (base64) => {
    var n = base64.length,
      bytes = new Uint8Array((((n - (base64[n - 1] == '=') - (base64[n - 2] == '=')) * 3) / 4) | 0);
    for (var i = 0, j = 0; i < n; ) {
      var c0 = table[base64.charCodeAt(i++)],
        c1 = table[base64.charCodeAt(i++)];
      var c2 = table[base64.charCodeAt(i++)],
        c3 = table[base64.charCodeAt(i++)];
      bytes[j++] = (c0 << 2) | (c1 >> 4);
      bytes[j++] = (c1 << 4) | (c2 >> 2);
      bytes[j++] = (c2 << 6) | c3;
    }
    return bytes;
  };
})();
