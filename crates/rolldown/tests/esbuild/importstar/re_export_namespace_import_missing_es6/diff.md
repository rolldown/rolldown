## /out.js
### esbuild
```js
// bar.js
var bar_exports = {};
__export(bar_exports, {
  x: () => x
});
var x = 123;

// entry.js
console.log(bar_exports, bar_exports.foo);
```
### rolldown
```js
import { default as assert } from "node:assert";


//#region bar.js
var bar_exports = {};
__export(bar_exports, { x: () => x });
const x = 123;

//#endregion
//#region entry.js
assert.deepEqual(bar_exports, { x: 123 });
assert.equal(bar_exports.foo, undefined);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry_js.mjs
@@ -1,4 +1,5 @@
 var bar_exports = {};
 __export(bar_exports, { x: () => x });
 var x = 123;
-console.log(bar_exports, bar_exports.foo);
\ No newline at end of file
+console.log(bar_exports);
+console.log(bar_exports.foo);
\ No newline at end of file

```
