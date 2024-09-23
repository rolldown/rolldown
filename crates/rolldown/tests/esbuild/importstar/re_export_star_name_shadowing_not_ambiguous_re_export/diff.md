## /out.js
### esbuild
```js
// b.js
var x = 1;

// entry.js
console.log(x);
```
### rolldown
```js
import { default as assert } from "node:assert";

//#region b.js
let x = 1;

//#endregion
//#region entry.js
assert.equal(x, 1);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry_js.mjs
@@ -1,2 +1,2 @@
-var x = 1;
+let x = 1;
 console.log(x);
\ No newline at end of file

```
