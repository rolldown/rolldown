# Reason
1. rolldown is not ts aware, it's not possible to support for now
2. sub optimal
# Diff
## /out.js
### esbuild
```js
// import.ts
var value = 123;

// entry.ts
var value_copy = value;
var foo = value_copy;
console.log(foo);
```
### rolldown
```js

//#region rolldown:runtime
var __defProp = Object.defineProperty;
var __export = (target, all) => {
	for (var name in all) __defProp(target, name, {
		get: all[name],
		enumerable: true
	});
};

//#region import.ts
var import_exports = {};
__export(import_exports, { value: () => value });
let value = 123;

//#region entry.ts
var value_copy = import_exports.value;
let foo = value_copy;
console.log(foo);

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,4 +1,15 @@
+var __defProp = Object.defineProperty;
+var __export = (target, all) => {
+    for (var name in all) __defProp(target, name, {
+        get: all[name],
+        enumerable: true
+    });
+};
+var import_exports = {};
+__export(import_exports, {
+    value: () => value
+});
 var value = 123;
-var value_copy = value;
+var value_copy = import_exports.value;
 var foo = value_copy;
 console.log(foo);

```