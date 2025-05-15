# Reason
1. different iife impl
# Diff
## /out.js
### esbuild
```js
var mod = (() => {
  var entry_exports = {};
  __export(entry_exports, {
    out: () => out
  });
  var out = __toESM(require("foo"));
  return __toCommonJS(entry_exports);
})();
```
### rolldown
```js
var mod = (function(exports, foo) {


foo = __toESM(foo);

Object.defineProperty(exports, 'out', {
  enumerable: true,
  get: function () {
    return foo;
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
+var mod = (function (exports, foo) {
+    foo = __toESM(foo);
+    Object.defineProperty(exports, 'out', {
+        enumerable: true,
+        get: function () {
+            return foo;
+        }
     });
-    var out = __toESM(require("foo"));
-    return __toCommonJS(entry_exports);
-})();
+    return exports;
+})({}, foo);

```