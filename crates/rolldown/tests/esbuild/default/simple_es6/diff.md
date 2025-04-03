# Diff
## /out.js
### esbuild
```js
// foo.js
function fn() {
  return 123;
}

// entry.js
console.log(fn());
```
### rolldown
```js
import assert from "node:assert";

//#region foo.js
function fn$1() {
	return 123;
}

//#endregion
//#region entry.js
assert.equal(fn$1(), 123);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,4 +1,4 @@
-function fn() {
+function fn$1() {
     return 123;
 }
-console.log(fn());
+console.log(fn$1());

```