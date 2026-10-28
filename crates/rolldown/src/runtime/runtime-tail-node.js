// This is created by the rolldown. It's used to polyfill the global `require` function when you bundling code
// with target `format: 'es'` and `platform: 'node'`. It's used to make sure the global `require` could
// works in Node.js esm environment.
// A shared runtime can live in another directory. Bind require to the calling chunk, not this
// helper's file. Keep one function per URL so require properties and identity stay stable.
// See internal-docs/code-splitting/implementation.md#runtime-module-placement.
var __nodeCreateRequire = createRequire;
var __nodeRequireCache = /* @__PURE__ */ __create(null);
export var __require = (url) =>
  __nodeRequireCache[url] || (__nodeRequireCache[url] = __nodeCreateRequire(url));
