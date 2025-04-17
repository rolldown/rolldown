# Reason
1. different file system
2. different naming style
# Diff
## /out.js
### esbuild
```js
// Users/user/project/src/index.js
var require_src = __commonJS({
  "Users/user/project/src/index.js"(exports, module) {
    module.exports = 123;
  }
});

// Users/user/project/src/dir/entry.js
console.log(require_src());
```
### rolldown
```js
import assert from "node:assert";

//#region rolldown:runtime
var __getOwnPropNames = Object.getOwnPropertyNames;
var __commonJS = (cb, mod) => function() {
	return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};

//#region index.js
var require_require_parent_dir_common_js = __commonJS({ "index.js"(exports, module) {
	module.exports = 123;
} });

//#region dir/entry.js
assert.deepEqual(require_require_parent_dir_common_js(), 123);

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	dir_entry.js
@@ -1,6 +1,12 @@
-var require_src = __commonJS({
-    "Users/user/project/src/index.js"(exports, module) {
+var __getOwnPropNames = Object.getOwnPropertyNames;
+var __commonJS = (cb, mod) => function () {
+    return (mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = {
+        exports: {}
+    }).exports, mod), mod.exports);
+};
+var require_require_parent_dir_common_js = __commonJS({
+    "index.js"(exports, module) {
         module.exports = 123;
     }
 });
-console.log(require_src());
+console.log(require_require_parent_dir_common_js());

```