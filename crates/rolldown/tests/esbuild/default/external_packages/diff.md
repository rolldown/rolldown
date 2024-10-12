# Diff
## /out.js
### esbuild
```js
// project/entry.js
import "pkg1";

// project/file.js
console.log("file");

// project/node_modules/pkg2/index.js
console.log("pkg2");

// project/libs/pkg3.js
console.log("pkg3");
```
### rolldown
```js
import "pkg1";
import "pkg2";

//#region file.js
console.log("file");

//#endregion
//#region libs/pkg3.js
console.log("pkg3");

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,4 +1,4 @@
 import "pkg1";
+import "pkg2";
 console.log("file");
-console.log("pkg2");
 console.log("pkg3");

```