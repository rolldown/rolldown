# Reason
1. different naming style
# Diff
## /out.js
### esbuild
```js
// Users/user/project/src/dir/index.js
var require_dir = __commonJS({
  "Users/user/project/src/dir/index.js"(exports, module) {
    module.exports = 123;
  }
});

// Users/user/project/src/entry.js
console.log(require_dir());
```
### rolldown
```js

//#region rolldown:runtime
var __getOwnPropNames = Object.getOwnPropertyNames;
var __commonJS = (cb, mod) => function() {
	return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};

//#region dir/index.js
var require_dir = __commonJS({ "dir/index.js"(exports, module) {
	module.exports = 123;
} });

//#region entry.js
console.log(require_dir());

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,6 +1,12 @@
+var __getOwnPropNames = Object.getOwnPropertyNames;
+var __commonJS = (cb, mod) => function () {
+    return (mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = {
+        exports: {}
+    }).exports, mod), mod.exports);
+};
 var require_dir = __commonJS({
-    "Users/user/project/src/dir/index.js"(exports, module) {
+    "dir/index.js"(exports, module) {
         module.exports = 123;
     }
 });
 console.log(require_dir());

```