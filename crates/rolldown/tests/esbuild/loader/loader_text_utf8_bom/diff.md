## /out.js
### esbuild
```js
// data1.txt
var data1_default = "text";

// data2.txt
var data2_default = "text\uFEFF";

// entry.js
console.log(data1_default, data2_default);
```
### rolldown
```js
//#region data1.txt
var data1_default = "\\xEF\\xBB\\xBFtext";

//#endregion
//#region data2.txt
var data2_default = "text\\xEF\\xBB\\xBF";

//#endregion
//#region entry.js
console.log(data1_default, data2_default);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,3 +1,3 @@
-var data1_default = "text";
-var data2_default = "text\uFEFF";
+var data1_default = "\\xEF\\xBB\\xBFtext";
+var data2_default = "text\\xEF\\xBB\\xBF";
 console.log(data1_default, data2_default);

```