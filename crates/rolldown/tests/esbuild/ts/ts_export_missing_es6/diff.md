# Reason
1. export missing es6
# Diff
## /out.js
### esbuild
```js
// foo.ts
var foo_exports = {};

// entry.js
console.log(foo_exports);
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

//#region bar.js
var nope = void 0;

//#region foo.ts
var foo_exports = {};
__export(foo_exports, { nope: () => nope });

//#region entry.js
console.log(foo_exports);

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,2 +1,13 @@
+var __defProp = Object.defineProperty;
+var __export = (target, all) => {
+    for (var name in all) __defProp(target, name, {
+        get: all[name],
+        enumerable: true
+    });
+};
+var nope = void 0;
 var foo_exports = {};
+__export(foo_exports, {
+    nope: () => nope
+});
 console.log(foo_exports);

```