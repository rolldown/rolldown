# Reason
1. rolldown implemented advanced barrel exports opt
# Diff
## /out.js
### esbuild
```js
// bar.js
var bar_exports = {};
__export(bar_exports, {
  x: () => x
});
var x = 123;

// entry.js
console.log(bar_exports, bar_exports.foo);
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

//#region bar.js
var bar_exports = {};
__export(bar_exports, { x: () => x });
const x = 123;

//#region entry.js
assert.deepEqual(bar_exports, { x: 123 });
assert.equal(void 0, void 0);

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,6 +1,13 @@
+var __defProp = Object.defineProperty;
+var __export = (target, all) => {
+    for (var name in all) __defProp(target, name, {
+        get: all[name],
+        enumerable: true
+    });
+};
 var bar_exports = {};
 __export(bar_exports, {
     x: () => x
 });
 var x = 123;
-console.log(bar_exports, bar_exports.foo);
+console.log(bar_exports, void 0);

```