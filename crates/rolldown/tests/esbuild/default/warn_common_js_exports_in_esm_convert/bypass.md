# Reason 
1.cjs module lexer can't recognize esbuild interop pattern
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
"use strict";

//#region cjs-in-esm.js
let foo = 1;
exports.foo = 2;
module.exports = 3;
//#endregion

exports.foo = foo
```
### diff
```diff
===================================================================
--- esbuild	/out/cjs-in-esm.js
+++ rolldown	cjs-in-esm.js
@@ -1,8 +1,4 @@
-var cjs_in_esm_exports = {};
-__export(cjs_in_esm_exports, {
-    foo: () => foo
-});
-module.exports = __toCommonJS(cjs_in_esm_exports);
-let foo = 1;
+var foo = 1;
 exports.foo = 2;
 module.exports = 3;
+exports.foo = foo;

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
"use strict";

//#region cjs-in-esm2.js
let foo = 1;
module.exports.bar = 3;
//#endregion

exports.foo = foo
```
### diff
```diff
===================================================================
--- esbuild	/out/cjs-in-esm2.js
+++ rolldown	cjs-in-esm2.js
@@ -1,7 +1,3 @@
-var cjs_in_esm2_exports = {};
-__export(cjs_in_esm2_exports, {
-    foo: () => foo
-});
-module.exports = __toCommonJS(cjs_in_esm2_exports);
-let foo = 1;
+var foo = 1;
 module.exports.bar = 3;
+exports.foo = foo;

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


const bar = __toESM(require("bar"));

//#region import-in-cjs.js
exports.foo = bar.foo;
module.exports = bar.foo;
module.exports.bar = bar.foo;
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/import-in-cjs.js
+++ rolldown	import-in-cjs.js
@@ -1,4 +1,4 @@
-var import_bar = require("bar");
-exports.foo = import_bar.foo;
-module.exports = import_bar.foo;
-module.exports.bar = import_bar.foo;
+var bar = __toESM(require("bar"));
+exports.foo = bar.foo;
+module.exports = bar.foo;
+module.exports.bar = bar.foo;

```