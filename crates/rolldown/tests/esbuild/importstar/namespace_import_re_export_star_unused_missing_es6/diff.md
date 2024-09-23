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
var foo_ns = {};
__export(foo_ns, { x: () => x });

//#endregion
//#region entry.js
assert.deepEqual(foo_ns, { x: 123 });

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
+const x = 123;
+var foo_ns = {};
+__export(foo_ns, { x: () => x });
+assert.deepEqual(foo_ns, { x: 123 });
\ No newline at end of file

```
