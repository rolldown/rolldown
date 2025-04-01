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



//#region index.js
var require_require_parent_dir_common_js = __commonJS({ "index.js"(exports, module) {
	module.exports = 123;
} });
//#endregion

//#region dir/entry.js
assert.deepEqual(require_require_parent_dir_common_js(), 123);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	dir_entry.js
@@ -1,6 +1,6 @@
-var require_src = __commonJS({
-    "Users/user/project/src/index.js"(exports, module) {
+var require_require_parent_dir_common_js = __commonJS({
+    "index.js"(exports, module) {
         module.exports = 123;
     }
 });
-console.log(require_src());
+console.log(require_require_parent_dir_common_js());

```