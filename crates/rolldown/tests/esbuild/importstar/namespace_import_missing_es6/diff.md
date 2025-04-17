# Diff
## /out.js
### esbuild
```js
// foo.js
var foo_exports = {};
__export(foo_exports, {
  x: () => x
});
var x = 123;

// entry.js
console.log(foo_exports, void 0);
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
var foo_exports = {};
__export(foo_exports, { x: () => x });
const x = 123;

//#region entry.js
assert.deepEqual(foo_exports, { x: 123 });
assert.equal(void 0, void 0);

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,4 +1,11 @@
+var __defProp = Object.defineProperty;
+var __export = (target, all) => {
+    for (var name in all) __defProp(target, name, {
+        get: all[name],
+        enumerable: true
+    });
+};
 var foo_exports = {};
 __export(foo_exports, {
     x: () => x
 });

```