# Diff
## /out.js
### esbuild
```js
// b.js
var require_b = __commonJS({
  "b.js"(exports) {
    exports.x = 123;
  }
});

// a.js
console.log(require_b());
console.log(require_b());
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,7 +0,0 @@
-var require_b = __commonJS({
-    "b.js"(exports) {
-        exports.x = 123;
-    }
-});
-console.log(require_b());
-console.log(require_b());

```