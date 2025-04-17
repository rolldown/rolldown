# Reason
1. `oxc` inject align with `@rollup/plugin-inject` don't support inject files directly
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