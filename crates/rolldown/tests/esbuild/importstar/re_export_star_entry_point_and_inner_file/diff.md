# Reason
1. rolldown has redundant `require('external')`
# Diff
## /out/entry.js
### esbuild
```js
// entry.js
var entry_exports = {};
__export(entry_exports, {
  inner: () => inner_exports
});
module.exports = __toCommonJS(entry_exports);
__reExport(entry_exports, require("a"), module.exports);

// inner.js
var inner_exports = {};
__reExport(inner_exports, require("b"));
```
### rolldown
```js
"use strict";


//#region a.js
const A = 1;
const B = "2";

//#endregion
//#region b.js
const C = 1;
const D = "2";

//#endregion
//#region inner.js
var inner_exports = {};
__export(inner_exports, {
	C: () => C,
	D: () => D
});

//#endregion
exports.A = A
exports.B = B
Object.defineProperty(exports, 'inner', {
  enumerable: true,
  get: function () {
    return inner_exports;
  }
});

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,8 +1,17 @@
-var entry_exports = {};
-__export(entry_exports, {
-    inner: () => inner_exports
-});
-module.exports = __toCommonJS(entry_exports);
-__reExport(entry_exports, require("a"), module.exports);
+var A = 1;
+var B = "2";
+var C = 1;
+var D = "2";
 var inner_exports = {};
-__reExport(inner_exports, require("b"));
+__export(inner_exports, {
+    C: () => C,
+    D: () => D
+});
+exports.A = A;
+exports.B = B;
+Object.defineProperty(exports, 'inner', {
+    enumerable: true,
+    get: function () {
+        return inner_exports;
+    }
+});

```
