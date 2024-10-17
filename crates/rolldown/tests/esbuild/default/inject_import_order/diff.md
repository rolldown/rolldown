# Diff
## /out.js
### esbuild
```js
// inject-1.js
import "first";
console.log("first");

// inject-2.js
import "second";
console.log("second");

// entry.ts
import "third";
console.log("third");
```
### rolldown
```js
import "third";

//#region entry.ts
console.log("third");

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,6 +1,2 @@
-import "first";
-console.log("first");
-import "second";
-console.log("second");
 import "third";
 console.log("third");

```