# Reason
1. different codegen order
# Diff
## /out.js
### esbuild
```js
(() => {
  // b.js
  var require_b = __commonJS({
    "b.js"(exports) {
      exports.x = 123;
    }
  });

  // a.js
  Promise.resolve().then(() => __toESM(require_b())).then((ns) => console.log(ns));
  Promise.resolve().then(() => __toESM(require_b())).then((ns) => console.log(ns));
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
@@ -1,9 +0,0 @@
-(() => {
-    var require_b = __commonJS({
-        "b.js"(exports) {
-            exports.x = 123;
-        }
-    });
-    Promise.resolve().then(() => __toESM(require_b())).then(ns => console.log(ns));
-    Promise.resolve().then(() => __toESM(require_b())).then(ns => console.log(ns));
-})();

```