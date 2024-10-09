# Diff
## /out/cjs-in-esm.js
### esbuild
```js
var cjs_in_esm_exports = {};
__export(cjs_in_esm_exports, {
  foo: () => foo
});
module.exports = __toCommonJS(cjs_in_esm_exports);
let foo = 1;
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
-let foo = 1;
-exports.foo = 2;
-module.exports = 3;

```
## /out/cjs-in-esm2.js
### esbuild
```js
var cjs_in_esm2_exports = {};
__export(cjs_in_esm2_exports, {
  foo: () => foo
});
module.exports = __toCommonJS(cjs_in_esm2_exports);
let foo = 1;
module.exports.bar = 3;
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/cjs-in-esm2.js
+++ rolldown	
@@ -1,7 +0,0 @@
-var cjs_in_esm2_exports = {};
-__export(cjs_in_esm2_exports, {
-    foo: () => foo
-});
-module.exports = __toCommonJS(cjs_in_esm2_exports);
-let foo = 1;
-module.exports.bar = 3;

```
## /out/import-in-cjs.js
### esbuild
```js
var import_bar = require("bar");
exports.foo = import_bar.foo;
module.exports = import_bar.foo;
module.exports.bar = import_bar.foo;
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/import-in-cjs.js
+++ rolldown	
@@ -1,4 +0,0 @@
-var import_bar = require("bar");
-exports.foo = import_bar.foo;
-module.exports = import_bar.foo;
-module.exports.bar = import_bar.foo;

```
## /out/no-warnings-here.js
### esbuild
```js
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