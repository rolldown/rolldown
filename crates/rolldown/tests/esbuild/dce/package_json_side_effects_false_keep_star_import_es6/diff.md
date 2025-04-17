# Diff
## /out.js
### esbuild
```js
// Users/user/project/node_modules/demo-pkg/index.js
var demo_pkg_exports = {};
__export(demo_pkg_exports, {
  foo: () => foo
});
var foo = 123;
console.log("hello");

// Users/user/project/src/entry.js
console.log(demo_pkg_exports);
```
### rolldown
```js
import assert from "node:assert";

//#region rolldown:runtime
var __defProp = Object.defineProperty;
var __export = (target, all) => {
	for (var name in all) __defProp(target, name, {
		get: all[name],
		enumerable: true
	});
};

//#region node_modules/demo-pkg/index.js
var demo_pkg_exports = {};
__export(demo_pkg_exports, { foo: () => foo });
const foo = 123;
console.log("hello");

//#region src/entry.js
assert.deepEqual(demo_pkg_exports, { foo: 123 });

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	src_entry.js
@@ -1,4 +1,11 @@
+var __defProp = Object.defineProperty;
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

```