# Diff
## /out.js
### esbuild
```js
var mod = (() => {
  // entry.js
  var entry_exports = {};
  __export(entry_exports, {
    out: () => out
  });
  var out = __toESM(__require("foo"));
  return __toCommonJS(entry_exports);
})();
```
### rolldown
```js
(function(exports, foo) {

"use strict";
const out = foo;

Object.defineProperty(exports, 'out', {
  enumerable: true,
  get: function () {
    return out;
  }
});
return exports;
})({}, foo);

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +1,10 @@
-var mod = (() => {
-    var entry_exports = {};
-    __export(entry_exports, {
-        out: () => out
+(function (exports, foo) {
+    const out = foo;
+    Object.defineProperty(exports, 'out', {
+        enumerable: true,
+        get: function () {
+            return out;
+        }
     });
-    var out = __toESM(__require("foo"));
-    return __toCommonJS(entry_exports);
-})();
+    return exports;
+})({}, foo);

```