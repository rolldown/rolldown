# Reason
1. not support conditional require
# Diff
## /out.js
### esbuild
```js
// b.js
var require_b = __commonJS({
  "b.js"(exports) {
    exports.foo = 213;
  }
});

// a.js
x ? __require("a") : y ? require_b() : __require("c");
x ? y ? __require("a") : require_b() : __require(c);
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
-        exports.foo = 213;
-    }
-});
-x ? __require("a") : y ? require_b() : __require("c");
-x ? y ? __require("a") : require_b() : __require(c);

```