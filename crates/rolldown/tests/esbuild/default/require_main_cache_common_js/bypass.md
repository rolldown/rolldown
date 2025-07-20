# Reason
1. trivial diff
# Diff
## /out.js
### esbuild
```js
// is-main.js
var require_is_main = __commonJS({
  "is-main.js"(exports2, module2) {
    module2.exports = require.main === module2;
  }
});

// entry.js
console.log("is main:", require.main === module);
console.log(require_is_main());
console.log("cache:", require.cache);
```
### rolldown
```js
// HIDDEN [rolldown:runtime]

//#region is-main.js
var require_is_main = __commonJS({ "is-main.js"(exports, module) {
	module.exports = require.main === module;
} });

//#endregion
//#region entry.js
console.log("is main:", require.main === module);
console.log(require_is_main());
console.log("cache:", require.cache);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,7 @@
 var require_is_main = __commonJS({
-    "is-main.js"(exports2, module2) {
-        module2.exports = require.main === module2;
+    "is-main.js"(exports, module) {
+        module.exports = require.main === module;
     }
 });
 console.log("is main:", require.main === module);
 console.log(require_is_main());

```