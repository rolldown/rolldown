## /out.js
### esbuild
```js
// entry.js
console.log(void 0);
```
### rolldown
```js
import { default as assert } from "node:assert";


//#region foo.js
var foo_ns = {};
__export(foo_ns, { x: () => x });
const x = 123;

//#endregion
//#region entry.js
assert.equal(foo_ns.foo, undefined);

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
+var foo_ns = {};
+__export(foo_ns, { x: () => x });
+const x = 123;
+assert.equal(foo_ns.foo, undefined);
\ No newline at end of file

```
