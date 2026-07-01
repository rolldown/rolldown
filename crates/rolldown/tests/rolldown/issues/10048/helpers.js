// A helper-only module: it defines and exports the esbuild-style CommonJS interop
// helpers (like storybook's chunk-IMSF75WX.js). This is the module rolldown
// wrongly tree-shakes away, orphaning the __commonJS / __toESM calls in theming.js.
var __getOwnPropNames = Object.getOwnPropertyNames;

var __commonJS = (cb, mod) => function () {
  return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};

var __toESM = (mod) => ({ default: mod });

export { __commonJS, __toESM };
