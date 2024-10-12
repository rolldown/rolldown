# Diff
## /out.js
### esbuild
```js
// data.json
var data_default = { works: true };

// data.json with { type: 'json' }
var data_default2 = { works: true };

// entry.js
console.log(data_default === data_default, data_default !== data_default2);
```
### rolldown
```js

//#region data.json
const works = true;
var data_default = { works };

//#endregion
//#region entry.js
console.log(data_default === data_default, data_default !== data_default);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,5 @@
+var works = true;
 var data_default = {
-    works: true
+    works
 };
-var data_default2 = {
-    works: true
-};
-console.log(data_default === data_default, data_default !== data_default2);
+console.log(data_default === data_default, data_default !== data_default);

```