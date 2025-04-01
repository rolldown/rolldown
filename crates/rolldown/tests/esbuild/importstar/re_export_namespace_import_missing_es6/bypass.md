# Reason
1. rolldown implemented advanced barrel exports opt
# Diff
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
import assert from "node:assert";



//#region bar.js
var bar_exports = {};
__export(bar_exports, { x: () => x });
const x = 123;
//#endregion

//#region entry.js
assert.deepEqual(bar_exports, { x: 123 });
assert.equal(void 0, void 0);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -2,5 +2,5 @@
 __export(bar_exports, {
     x: () => x
 });
 var x = 123;
-console.log(bar_exports, bar_exports.foo);
+console.log(bar_exports, void 0);

```