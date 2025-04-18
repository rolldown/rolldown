# Reason
1. different file system
2. different naming style
# Diff
## /out.js
### esbuild
```js
// Users/user/project/node_modules/demo-pkg/index.js
var require_demo_pkg = __commonJS({
  "Users/user/project/node_modules/demo-pkg/index.js"(exports) {
    exports.foo = 123;
    console.log("hello");
  }
});

// Users/user/project/src/entry.js
require_demo_pkg();
console.log("unused import");
```
### rolldown
```js

//#region node_modules/demo-pkg/index.js
var require_demo_pkg = __commonJS({ "node_modules/demo-pkg/index.js"(exports) {
	exports.foo = 123;
	console.log("hello");
} });

//#endregion
//#region src/entry.js
require_demo_pkg();
console.log("unused import");

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	src_entry.js
@@ -1,6 +1,6 @@
 var require_demo_pkg = __commonJS({
-    "Users/user/project/node_modules/demo-pkg/index.js"(exports) {
+    "node_modules/demo-pkg/index.js"(exports) {
         exports.foo = 123;
         console.log("hello");
     }
 });

```