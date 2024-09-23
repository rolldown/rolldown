## /out.js
### esbuild
```js
// foo.js
var x = 1;

// bar.js
var z = 4;

// entry.js
console.log(x, void 0, z);
```
### rolldown
```js
import { default as assert } from "node:assert";

//#region foo.js
const x = 1;

//#endregion
//#region bar.js
const z = 4;

//#endregion
//#region entry.js
assert.equal(x, 1);
assert.equal(void 0, undefined);
assert.equal(z, 4);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry_js.mjs
@@ -1,3 +1,5 @@
-var x = 1;
-var z = 4;
-console.log(x, void 0, z);
\ No newline at end of file
+const x = 1;
+const z = 4;
+console.log(x);
+console.log(void 0);
+console.log(z);
\ No newline at end of file

```
