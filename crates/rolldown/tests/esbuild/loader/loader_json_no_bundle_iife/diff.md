# Reason
1. Wrong output
# Diff
## /out.js
### esbuild
```js
(() => {
  var require_test = __commonJS({
    "test.json"(exports, module) {
      module.exports = { test: 123, "invalid-identifier": true };
    }
  });
  require_test();
})();
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,11 +0,0 @@
-(() => {
-    var require_test = __commonJS({
-        "test.json"(exports, module) {
-            module.exports = {
-                test: 123,
-                "invalid-identifier": true
-            };
-        }
-    });
-    require_test();
-})();

```