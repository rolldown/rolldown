#### In-depth

##### ES Module

[ES modules](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Guide/Modules) (ESM) are the official JavaScript module standard. When `output.format: 'esm'` is used, the bundle will use `export` syntax like this:

```js
function exportedFunction() {
  /* ... */
}
let exportedValue = '/* ... */';

export { exportedFunction, exportedValue };
```

To load ES modules, use `<script type="module">` in browsers, or `.mjs` extension (or `"type": "module"` in package.json) in Node.js. See [Node.js ES modules documentation](https://nodejs.org/api/esm.html#enabling) for details.

ES modules are the recommended format for most use cases. They are part of the JavaScript specification, work across browsers and Node.js, and enable static analysis for tree-shaking and other optimizations.

##### CommonJS Module

[CommonJS](https://nodejs.org/docs/latest/api/modules.html#modules-commonjs-modules) is the module format that Node.js originally supported before ES modules. When `output.format: 'cjs'` is used, the bundle will use `exports` variable like this:

```js
function exportedFunction() {
  /* ... */
}
let exportedValue = '/* ... */';

exports.exportedFunction = exportedFunction;
exports.exportedValue = exportedValue;
```

Entry points with ES module exports will be converted to use getters on `exports` to preserve [live binding semantics](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Statements/import#imported_values_can_only_be_modified_by_the_exporter).

CommonJS is useful when targeting Node.js environments that don't support ES modules, or when integrating with older packages that expect CommonJS. For most use cases, `esm` format is preferred because it's the JavaScript standard, has better interoperability across environments, and enables static analysis for tree-shaking.

##### IIFE

IIFE stands for ["immediately-invoked function expression"](https://developer.mozilla.org/en-US/docs/Glossary/IIFE). When `output.format: 'iife'` is used, the bundle will be wrapped with an IIFE like this (assuming [`output.name: 'MyLibrary'`](/reference/OutputOptions.name) is set):

```js
var MyLibrary = (function () {
  function exportedFunction() {
    /* ... */
  }
  let exportedValue = '/* ... */';

  return { exportedFunction, exportedValue };
})();
```

When using `<script>` tags without `type="module"`, code executes in the global scope, which means variables from different scripts can conflict with each other. IIFE solves this by creating a private function scope that encapsulates all internal variables, while exposing only a single global variable (e.g., `jQuery`, `_`, `React`).

IIFE is useful for drop-in scripts and widgets that need to work anywhere with a single `<script>` tag, or for libraries that want to expose one clean global (like analytics snippets or embeddable widgets). For most use cases, especially libraries, `esm` format is preferred because it's the JavaScript standard, has better interoperability across environments, and enables static analysis for tree-shaking.

##### UMD

[Universal Module Definition](https://github.com/umdjs/umd) is a pattern that works across multiple environments: [AMD](https://github.com/amdjs/amdjs-api/blob/master/AMD.md) (RequireJS), CommonJS (Node.js), and browser globals. When `output.format: 'umd'` is used, the bundle will be wrapped with code that detects the environment like this (assuming [`output.name: 'MyLibrary'`](/reference/OutputOptions.name) is set):

```js
(function (global, factory) {
  typeof exports === 'object' && typeof module !== 'undefined'
    ? factory(exports)
    : typeof define === 'function' && define.amd
      ? define(['exports'], factory)
      : ((global = typeof globalThis !== 'undefined' ? globalThis : global || self),
        factory((global.myBundle = {})));
})(this, function (exports) {
  function exportedFunction() {
    /* ... */
  }
  let exportedValue = '/* ... */';

  exports.exportedFunction = exportedFunction;
  exports.exportedValue = exportedValue;
});
```

UMD was popular before ES modules became widely supported, as it allowed a single build to work everywhere. Today, UMD is largely unnecessary as ES modules are supported in all modern browsers and Node.js, and bundlers handle module interop automatically. The format also adds runtime overhead and is harder to statically analyze. For new projects, use `esm` format instead.
