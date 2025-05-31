# Reason
1. different file system
# Diff
## /out.js
### esbuild
```js
// Users/user/project/node_modules/demo-pkg/index.js
var demo_pkg_exports = {};
__export(demo_pkg_exports, {
  foo: () => foo
});
var foo;
var init_demo_pkg = __esm({
  "Users/user/project/node_modules/demo-pkg/index.js"() {
    foo = 123;
    console.log("hello");
  }
});

// Users/user/project/src/entry.js
init_demo_pkg();
console.log("unused import");
```
### rolldown
```js

//#region node_modules/demo-pkg/index.js
var demo_pkg_exports = {};
__export(demo_pkg_exports, { foo: () => foo });
const foo = 123;
var init_demo_pkg = __esm({ "node_modules/demo-pkg/index.js"() {
	console.log("hello");
} });

//#endregion
//#region src/entry.js
init_demo_pkg();
console.log("unused import");

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	src_entry.js
@@ -1,12 +1,11 @@
 var demo_pkg_exports = {};
 __export(demo_pkg_exports, {
     foo: () => foo
 });
-var foo;
+var foo = 123;
 var init_demo_pkg = __esm({
-    "Users/user/project/node_modules/demo-pkg/index.js"() {
-        foo = 123;
+    "node_modules/demo-pkg/index.js"() {
         console.log("hello");
     }
 });
 init_demo_pkg();

```