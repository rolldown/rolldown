## /out.js
### esbuild
```js
var someName = (() => {
  // entry.js
  var entry_exports = {};
  __export(entry_exports, {
    foo: () => foo
  });
  var foo = 123;
  return __toCommonJS(entry_exports);
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
@@ -1,8 +1,10 @@
-var someName = (() => {
-    var entry_exports = {};
-    __export(entry_exports, {
-        foo: () => foo
+(function (exports) {
+    const foo = 123;
+    Object.defineProperty(exports, 'foo', {
+        enumerable: true,
+        get: function () {
+            return foo;
+        }
     });
-    var foo = 123;
-    return __toCommonJS(entry_exports);
-})();
+    return exports;
+})({});

```
