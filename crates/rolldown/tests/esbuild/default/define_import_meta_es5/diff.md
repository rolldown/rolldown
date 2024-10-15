# Reason
1. should warn when target do not support `imoprt.meta`
# Diff
## /out/kept.js
### esbuild
```js
// kept.js
var import_meta = {};
console.log(import_meta.y);
```
### rolldown
```js

//#region kept.js
console.log(import.meta.y);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/kept.js
+++ rolldown	kept.js
@@ -1,2 +1,1 @@
-var import_meta = {};
-console.log(import_meta.y);
+console.log(import.meta.y);

```