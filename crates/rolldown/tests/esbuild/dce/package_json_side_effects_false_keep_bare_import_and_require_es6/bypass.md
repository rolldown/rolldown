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

//#region node_modules/demo-pkg/index.js
var demo_pkg_exports = {};
__export(demo_pkg_exports, { foo: () => foo });
var foo;
var init_demo_pkg = __esm({ "node_modules/demo-pkg/index.js"() {
	foo = 123;
	console.log("hello");
} });

//#region src/entry.js
init_demo_pkg();
console.log("unused import");

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	src_entry.js
@@ -1,11 +1,22 @@
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
 var demo_pkg_exports = {};
 __export(demo_pkg_exports, {
     foo: () => foo
 });
 var foo;
 var init_demo_pkg = __esm({
-    "Users/user/project/node_modules/demo-pkg/index.js"() {
+    "node_modules/demo-pkg/index.js"() {
         foo = 123;
         console.log("hello");
     }
 });

```