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

//#region dir/index.js
var require_dir = __commonJS({ "dir/index.js"(exports, module) {
	module.exports = 123;
} });

//#endregion
//#region entry.js
console.log(require_dir());

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,6 +1,6 @@
 var require_dir = __commonJS({
-    "Users/user/project/src/dir/index.js"(exports, module) {
+    "dir/index.js"(exports, module) {
         module.exports = 123;
     }
 });
 console.log(require_dir());

```