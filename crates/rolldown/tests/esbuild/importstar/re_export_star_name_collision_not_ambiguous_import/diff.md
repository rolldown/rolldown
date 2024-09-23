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
import { default as assert } from "node:assert";

//#region c.js
let x = 1;
let y = 2;

//#endregion
//#region entry.js
assert(x === 1 && y === 2);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry_js.mjs
@@ -1,3 +1,3 @@
-var x = 1;
-var y = 2;
-console.log(x, y);
\ No newline at end of file
+let x = 1;
+let y = 2;
+assert(x === 1 && y === 2);
\ No newline at end of file

```
