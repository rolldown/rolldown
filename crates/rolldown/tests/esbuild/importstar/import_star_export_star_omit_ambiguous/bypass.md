# Reason
1. different deconflict order
# Diff
## /out.js
### esbuild
```js
// common.js
var common_exports = {};
__export(common_exports, {
  x: () => x,
  z: () => z
});

// foo.js
var x = 1;

// bar.js
var z = 4;

// entry.js
console.log(common_exports);
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

//#region foo.js
const x = 1;

//#region bar.js
const z = 4;

//#region common.js
var common_exports = {};
__export(common_exports, {
	x: () => x,
	z: () => z
});

//#region entry.js
assert.deepEqual(common_exports, {
	x: 1,
	z: 4
});

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +1,15 @@
+var __defProp = Object.defineProperty;
+var __export = (target, all) => {
+    for (var name in all) __defProp(target, name, {
+        get: all[name],
+        enumerable: true
+    });
+};
+var x = 1;
+var z = 4;
 var common_exports = {};
 __export(common_exports, {
     x: () => x,
     z: () => z
 });
-var x = 1;
-var z = 4;
 console.log(common_exports);

```