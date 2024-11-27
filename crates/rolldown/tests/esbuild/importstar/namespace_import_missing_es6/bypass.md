# Reason
1. rolldown generates module namespace object in the bottom if possible.
# Diff
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
import assert from "node:assert";


//#region foo.js
const x = 123;
var foo_exports = {};
__export(foo_exports, { x: () => x });

//#endregion
//#region entry.js
assert.deepEqual(foo_exports, { x: 123 });
assert.equal(void 0, undefined);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,6 +1,6 @@
+var x = 123;
 var foo_exports = {};
 __export(foo_exports, {
     x: () => x
 });
-var x = 123;
 console.log(foo_exports, void 0);

```