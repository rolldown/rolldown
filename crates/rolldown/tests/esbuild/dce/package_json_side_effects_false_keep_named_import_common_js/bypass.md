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
var import_demo_pkg = __toESM(require_demo_pkg());
console.log(import_demo_pkg.foo);
```
### rolldown
```js


//#region node_modules/demo-pkg/index.js
var import_demo_pkg;
var require_demo_pkg = __commonJS({ "node_modules/demo-pkg/index.js"(exports) {
	exports.foo = 123;
	console.log("hello");
	import_demo_pkg = __toESM(require_demo_pkg());
} });

//#endregion
//#region src/entry.js
require_demo_pkg();
console.log(import_demo_pkg.foo);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	src_entry.js
@@ -1,8 +1,10 @@
+var import_demo_pkg;
 var require_demo_pkg = __commonJS({
-    "Users/user/project/node_modules/demo-pkg/index.js"(exports) {
+    "node_modules/demo-pkg/index.js"(exports) {
         exports.foo = 123;
         console.log("hello");
+        import_demo_pkg = __toESM(require_demo_pkg());
     }
 });
-var import_demo_pkg = __toESM(require_demo_pkg());
+require_demo_pkg();
 console.log(import_demo_pkg.foo);

```