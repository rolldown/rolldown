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
console.log(bar_exports.foo);
```
### rolldown
```js
import assert from "node:assert";

//#region entry.js
assert.deepEqual(void 0, void 0);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,6 +1,1 @@
-var bar_exports = {};
-__export(bar_exports, {
-    x: () => x
-});
-var x = 123;
-console.log(bar_exports.foo);
+console.log(void 0);

```