// This fallback "require" function exists so that "typeof require" can
// naturally be "function" even in non-CommonJS environments since esbuild
// emulates a CommonJS environment (issue #1202). However, people want this
// shim to fall back to "globalThis.require" even if it's defined later
// (including property accesses such as "require.resolve") so we need to
// use a proxy (issue #1614).
export var __require = /* @__PURE__ */ (x =>
  typeof require !== 'undefined' ? require :
    typeof Proxy !== 'undefined' ? new Proxy(x, {
      get: (a, b) => (typeof require !== 'undefined' ? require : a)[b]
    }) : x
)(function (x) {
  if (typeof require !== 'undefined') return require.apply(this, arguments)
  throw Error('Calling `require` for "' + x + '" in an environment that doesn\'t expose the `require` function.')
});
