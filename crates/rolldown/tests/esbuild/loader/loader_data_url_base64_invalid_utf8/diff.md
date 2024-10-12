# Diff
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

//#region binary.txt
var binary_default = "data:text/plain;base64,/w==";

//#endregion
//#region entry.js
console.log(binary_default);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,2 +1,2 @@
-var binary_default = "data:text/plain;charset=utf-8;base64,/w==";
+var binary_default = "data:text/plain;base64,/w==";
 console.log(binary_default);

```