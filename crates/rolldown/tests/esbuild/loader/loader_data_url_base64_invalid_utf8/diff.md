## /out.js
### esbuild
```js
// binary.txt
var binary_default = "data:text/plain;charset=utf-8;base64,/w==";

// entry.js
console.log(binary_default);
```
### rolldown
```js
//#endregion
//#region entry.js
console.log("data:application/octet-stream;base64,/w==");
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,2 +1,1 @@
-var binary_default = "data:text/plain;charset=utf-8;base64,/w==";
-console.log(binary_default);
+console.log("data:application/octet-stream;base64,/w==");

```