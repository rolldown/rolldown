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
Object.defineProperty(exports, 'foo', {
  enumerable: true,
  get: function () {
    return foo;
  }
});
return exports;
})({});

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry_js.mjs
@@ -1,3 +1,10 @@
-(() => {
-    var foo = 123;
-})();
\ No newline at end of file
+(function (exports) {
+    const foo = 123;
+    Object.defineProperty(exports, 'foo', {
+        enumerable: true,
+        get: function () {
+            return foo;
+        }
+    });
+    return exports;
+}({}));
\ No newline at end of file

```
