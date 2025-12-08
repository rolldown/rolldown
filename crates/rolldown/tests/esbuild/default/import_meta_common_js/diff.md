## /out.js
### esbuild
```js
// entry.js
var import_meta = {};
console.log(import_meta.url, import_meta.path);
```
### rolldown
```js

//#region entry.js
console.log(require("url").pathToFileURL(__filename).href, {}.path);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,2 +1,1 @@
-var import_meta = {};
-console.log(import_meta.url, import_meta.path);
+console.log(require("url").pathToFileURL(__filename).href, ({}).path);

```