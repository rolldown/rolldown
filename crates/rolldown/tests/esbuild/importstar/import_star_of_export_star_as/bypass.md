# Reason
1. different deconflict order
# Diff
## /out.js
### esbuild
```js
// foo.js
var foo_exports = {};
__export(foo_exports, {
  bar_ns: () => bar_exports
});

// bar.js
var bar_exports = {};
__export(bar_exports, {
  bar: () => bar
});
var bar = 123;

// entry.js
console.log(foo_exports);
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
__export(bar_exports, { bar: () => bar });
const bar = 123;

//#region foo.js
var foo_exports = {};
__export(foo_exports, { bar_ns: () => bar_exports });

//#region entry.js
console.log(foo_exports);
assert.deepEqual(foo_exports, { bar_ns: { bar: 123 } });

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,10 +1,18 @@
-var foo_exports = {};
-__export(foo_exports, {
-    bar_ns: () => bar_exports
-});
+var __defProp = Object.defineProperty;
+var __export = (target, all) => {
+    for (var name in all) __defProp(target, name, {
+        get: all[name],
+        enumerable: true
+    });
+};
 var bar_exports = {};
 __export(bar_exports, {
     bar: () => bar
 });
 var bar = 123;
+var foo_exports = {};
+__export(foo_exports, {
+    bar_ns: () => bar_exports
+});
 console.log(foo_exports);
+console.log(foo_exports);

```