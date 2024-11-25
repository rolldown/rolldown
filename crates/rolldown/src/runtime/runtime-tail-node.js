// This is created by the rolldown. It's used to polyfill the global `require` function when you bundling code
// with target `format: 'esm'` and `platform: 'node'`. It's used to make sure the global `require` could 
// works in Node.js esm environment.
export var __require = /* @__PURE__ */ createRequire(import.meta.url);