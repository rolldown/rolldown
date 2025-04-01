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



//#region foo.js
const x = 1;
//#endregion

//#region bar.js
const z = 4;
//#endregion

//#region common.js
var common_exports = {};
__export(common_exports, {
	x: () => x,
	z: () => z
});
//#endregion

//#region entry.js
assert.deepEqual(common_exports, {
	x: 1,
	z: 4
});
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +1,8 @@
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