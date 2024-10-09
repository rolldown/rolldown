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

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,6 +0,0 @@
-import "first";
-console.log("first");
-import "second";
-console.log("second");
-import "third";
-console.log("third");

```