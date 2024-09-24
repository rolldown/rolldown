## /out.js
### esbuild
```js
// entry.js
console.log(void 0);
```
### rolldown
```js
import { default as assert } from "node:assert";


//#region bar.js
const x = 123;

//#endregion
//#region foo.js
var foo_exports = {};
__export(foo_exports, { x: () => x });

//#endregion
//#region entry.js
assert.deepEqual(foo_exports, { x: 123 });

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry_js.mjs
@@ -1,1 +1,4 @@
-console.log(void 0);
\ No newline at end of file
+var x = 123;
+var foo_exports = {};
+__export(foo_exports, { x: () => x });
+console.log(foo_exports);
\ No newline at end of file

```
