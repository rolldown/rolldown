# Reason
1. this undefined
# Diff
## /out/entry.js
### esbuild
```js
// file1.js
var file1_default = [void 0, void 0];

// node_modules/pkg/file2.js
var file2_default = [void 0, void 0];

// entry.js
console.log(file1_default, file2_default);
```
### rolldown
```js

//#region file1.js
var file1_default = [this, this];

//#endregion
//#region node_modules/pkg/file2.js
var file2_default = [this, this];

//#endregion
//#region entry.js
console.log(file1_default, file2_default);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,3 +1,3 @@
-var file1_default = [void 0, void 0];
-var file2_default = [void 0, void 0];
+var file1_default = [this, this];
+var file2_default = [this, this];
 console.log(file1_default, file2_default);

```