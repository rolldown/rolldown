## /out.js
### esbuild
```js
// foo.js
var foo_exports = {};
__export(foo_exports, {
  x: () => x
});

// bar.js
var x = 123;

// entry.js
console.log(foo_exports, void 0);
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
assert.equal(foo_exports.foo, undefined);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry_js.mjs
@@ -1,4 +1,5 @@
+var x = 123;
 var foo_exports = {};
 __export(foo_exports, { x: () => x });
-var x = 123;
-console.log(foo_exports, void 0);
\ No newline at end of file
+console.log(foo_exports);
+console.log(foo_exports.foo);
\ No newline at end of file

```
