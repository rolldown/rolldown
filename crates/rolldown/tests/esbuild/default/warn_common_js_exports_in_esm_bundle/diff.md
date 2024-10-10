# Diff
## /out/cjs-in-esm.js
### esbuild
```js
// cjs-in-esm.js
var cjs_in_esm_exports = {};
__export(cjs_in_esm_exports, {
  foo: () => foo
});
module.exports = __toCommonJS(cjs_in_esm_exports);
var foo = 1;
exports.foo = 2;
module.exports = 3;
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/cjs-in-esm.js
+++ rolldown	
@@ -1,8 +0,0 @@
-var cjs_in_esm_exports = {};
-__export(cjs_in_esm_exports, {
-    foo: () => foo
-});
-module.exports = __toCommonJS(cjs_in_esm_exports);
-var foo = 1;
-exports.foo = 2;
-module.exports = 3;

```
## /out/import-in-cjs.js
### esbuild
```js
// import-in-cjs.js
var import_bar = require("bar");
exports.foo = import_bar.foo;
module.exports = import_bar.foo;
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/import-in-cjs.js
+++ rolldown	
@@ -1,3 +0,0 @@
-var import_bar = require("bar");
-exports.foo = import_bar.foo;
-module.exports = import_bar.foo;

```
## /out/no-warnings-here.js
### esbuild
```js
// no-warnings-here.js
console.log(module, exports);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/no-warnings-here.js
+++ rolldown	
@@ -1,1 +0,0 @@
-console.log(module, exports);

```