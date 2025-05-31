# Reason
1. double module initialization
# Diff
## /out.js
### esbuild
```js
// Users/user/project/node_modules/demo-pkg/index-main.js
var index_main_exports = {};
__export(index_main_exports, {
  foo: () => foo
});
var foo;
var init_index_main = __esm({
  "Users/user/project/node_modules/demo-pkg/index-main.js"() {
    foo = 123;
    console.log("this should be kept");
  }
});

// Users/user/project/src/require-demo-pkg.js
init_index_main();

// Users/user/project/src/entry.js
console.log("unused import");
```
### rolldown
```js

//#region node_modules/demo-pkg/index-module.js
var index_module_exports = {};
__export(index_module_exports, { foo: () => foo });
const foo = 123;
var init_index_module = __esm({ "node_modules/demo-pkg/index-module.js"() {
	console.log("TEST FAILED");
} });

//#endregion
//#region src/require-demo-pkg.js
init_index_module();

//#endregion
//#region src/entry.js
init_index_module();
console.log("unused import");

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	src_entry.js
@@ -1,13 +1,13 @@
-var index_main_exports = {};
-__export(index_main_exports, {
+var index_module_exports = {};
+__export(index_module_exports, {
     foo: () => foo
 });
-var foo;
-var init_index_main = __esm({
-    "Users/user/project/node_modules/demo-pkg/index-main.js"() {
-        foo = 123;
-        console.log("this should be kept");
+var foo = 123;
+var init_index_module = __esm({
+    "node_modules/demo-pkg/index-module.js"() {
+        console.log("TEST FAILED");
     }
 });
-init_index_main();
+init_index_module();
+init_index_module();
 console.log("unused import");

```