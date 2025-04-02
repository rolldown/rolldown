# Diff
## /out.js
### esbuild
```js
// a.js
var x = 1;

// entry.js
console.log(x);
```
### rolldown
```js
import assert from "node:assert";

//#region a.js
let x$1 = 1;

//#endregion
//#region entry.js
assert.equal(x$1, 1);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,2 +1,2 @@
-var x = 1;
-console.log(x);
+var x$1 = 1;
+console.log(x$1);

```