# Reason
1. Wrong impl
# Diff
## /out.js
### esbuild
```js
var mod = (() => {
  // entry.js
  var entry_exports = {};
  __reExport(entry_exports, __require("foo"));
  return __toCommonJS(entry_exports);
})();
```
### rolldown
```js
(function() {



var foo = require("foo");
Object.keys(foo).forEach(function (k) {
  if (k !== 'default' && !Object.prototype.hasOwnProperty.call(exports, k)) Object.defineProperty(exports, k, {
    enumerable: true,
    get: function () { return foo[k]; }
  });
});

})();
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,5 +1,11 @@
-var mod = (() => {
-    var entry_exports = {};
-    __reExport(entry_exports, __require("foo"));
-    return __toCommonJS(entry_exports);
+(function () {
+    var foo = require("foo");
+    Object.keys(foo).forEach(function (k) {
+        if (k !== 'default' && !Object.prototype.hasOwnProperty.call(exports, k)) Object.defineProperty(exports, k, {
+            enumerable: true,
+            get: function () {
+                return foo[k];
+            }
+        });
+    });
 })();

```