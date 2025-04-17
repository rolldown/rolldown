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

// Users/user/project/src/entry.js
init_index_main();

// Users/user/project/src/require-demo-pkg.js
init_index_main();

// Users/user/project/src/entry.js
console.log("unused import");
```
### rolldown
```js

//#region rolldown:runtime
var __defProp = Object.defineProperty;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __esm = (fn, res) => function() {
	return fn && (res = (0, fn[__getOwnPropNames(fn)[0]])(fn = 0)), res;
};
var __export = (target, all) => {
	for (var name in all) __defProp(target, name, {
		get: all[name],
		enumerable: true
	});
};

//#region node_modules/demo-pkg/index-module.js
var index_module_exports = {};
__export(index_module_exports, { foo: () => foo });
var foo;
var init_index_module = __esm({ "node_modules/demo-pkg/index-module.js"() {
	foo = 123;
	console.log("TEST FAILED");
} });

//#region src/require-demo-pkg.js
init_index_module();

//#region src/entry.js
init_index_module();
console.log("unused import");

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	src_entry.js
@@ -1,14 +1,25 @@
-var index_main_exports = {};
-__export(index_main_exports, {
+var __defProp = Object.defineProperty;
+var __getOwnPropNames = Object.getOwnPropertyNames;
+var __esm = (fn, res) => function () {
+    return (fn && (res = (0, fn[__getOwnPropNames(fn)[0]])(fn = 0)), res);
+};
+var __export = (target, all) => {
+    for (var name in all) __defProp(target, name, {
+        get: all[name],
+        enumerable: true
+    });
+};
+var index_module_exports = {};
+__export(index_module_exports, {
     foo: () => foo
 });
 var foo;
-var init_index_main = __esm({
-    "Users/user/project/node_modules/demo-pkg/index-main.js"() {
+var init_index_module = __esm({
+    "node_modules/demo-pkg/index-module.js"() {
         foo = 123;
-        console.log("this should be kept");
+        console.log("TEST FAILED");
     }
 });
-init_index_main();
-init_index_main();
+init_index_module();
+init_index_module();
 console.log("unused import");

```