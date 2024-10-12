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
import { default as assert } from "node:assert";

//#region foo.js
function fn() {
	return 123;
}

//#endregion
//#region entry.js
assert(fn() === 123);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,4 +1,4 @@
 function fn() {
     return 123;
 }
-console.log(fn());
+assert(fn() === 123);

```