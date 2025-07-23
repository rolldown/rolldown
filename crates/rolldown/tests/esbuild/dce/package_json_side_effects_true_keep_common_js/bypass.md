# Reason
1. different file system
2. rolldown now support commonjs tree shaking
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
console.log("unused import");
```
### rolldown
```js
// HIDDEN [rolldown:runtime]
//#region node_modules/demo-pkg/index.js
var require_demo_pkg = /* @__PURE__ */ __commonJS({ "node_modules/demo-pkg/index.js"(exports) {
	console.log("hello");
} });

//#endregion
//#region src/entry.js
var import_demo_pkg = __toESM(require_demo_pkg());
console.log("unused import");

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	src_entry.js
@@ -1,7 +1,6 @@
 var require_demo_pkg = __commonJS({
-    "Users/user/project/node_modules/demo-pkg/index.js"(exports) {
-        exports.foo = 123;
+    "node_modules/demo-pkg/index.js"(exports) {
         console.log("hello");
     }
 });
 var import_demo_pkg = __toESM(require_demo_pkg());

```