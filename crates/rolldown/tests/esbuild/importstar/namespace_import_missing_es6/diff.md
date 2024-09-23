## /out.js
### esbuild
```js
// foo.js
var foo_exports = {};
__export(foo_exports, {
  x: () => x
});
var x = 123;

// entry.js
console.log(foo_exports, void 0);
```
### rolldown
```js
import { default as assert } from "node:assert";


//#region foo.js
var foo_exports = {};
__export(foo_exports, { x: () => x });
const x = 123;

//#endregion
//#region entry.js
assert.equal(foo_exports.foo, undefined);
assert.deepEqual(foo_exports, { x: 123 });

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry_js.mjs
@@ -1,4 +1,5 @@
 var foo_exports = {};
 __export(foo_exports, { x: () => x });
-var x = 123;
-console.log(foo_exports, void 0);
\ No newline at end of file
+const x = 123;
+console.log(foo_exports.foo);
+assert.deepEqual(foo_exports, { x: 123 });
\ No newline at end of file

```
