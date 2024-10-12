# Diff
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
exports.foo = foo
return exports;
})({});

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +1,5 @@
-var someName = (() => {
-    var entry_exports = {};
-    __export(entry_exports, {
-        foo: () => foo
-    });
-    var foo = 123;
-    return __toCommonJS(entry_exports);
-})();
+(function (exports) {
+    const foo = 123;
+    exports.foo = foo;
+    return exports;
+})({});

```