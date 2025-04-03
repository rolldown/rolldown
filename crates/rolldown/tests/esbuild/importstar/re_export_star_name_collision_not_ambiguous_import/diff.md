# Diff
## /out.js
### esbuild
```js
// c.js
var x = 1;
var y = 2;

// entry.js
console.log(x, y);
```
### rolldown
```js
import assert from "node:assert";

//#region c.js
let x$1 = 1;
let y$1 = 2;

//#endregion
//#region entry.js
assert.equal(x$1, 1);
assert.equal(y$1, 2);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,3 +1,3 @@
-var x = 1;
-var y = 2;
-console.log(x, y);
+var x$1 = 1;
+var y$1 = 2;
+console.log(x$1, y$1);

```