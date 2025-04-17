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
//#region rolldown:runtime
var __getOwnPropNames = Object.getOwnPropertyNames;
var __commonJS = (cb, mod) => function() {
	return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};


//#region is-main.js
var require_is_main = __commonJS({ "is-main.js"(exports, module) {
	module.exports = require.main === module;
} });

//#region entry.js
console.log("is main:", require.main === module);
console.log(require_is_main());
console.log("cache:", require.cache);

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,13 @@
+var __getOwnPropNames = Object.getOwnPropertyNames;
+var __commonJS = (cb, mod) => function () {
+    return (mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = {
+        exports: {}
+    }).exports, mod), mod.exports);
+};
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