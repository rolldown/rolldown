# Reason 
1. Rolldown iife impl follow rollup
# Diff
## /out.js
### esbuild
```js
(() => {
  // entry.js
  var foo = 123;
})();
```
### rolldown
```js
(function(exports) {

"use strict";

//#region entry.js
const foo = 123;

//#endregion
exports.foo = foo;
return exports;
})({});
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,3 +1,5 @@
-(() => {
-    var foo = 123;
-})();
+(function (exports) {
+    const foo = 123;
+    exports.foo = foo;
+    return exports;
+})({});

```