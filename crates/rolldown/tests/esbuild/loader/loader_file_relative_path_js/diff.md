# Reason
1. abs output base
# Diff
## /out/entries/entry.js
### esbuild
```js
// src/images/image.png
var image_default = "../image-LSAMBFUD.png";

// src/entries/entry.js
console.log(image_default);
```
### rolldown
```js

//#region images/image.png
var image_default = "assets/image-3GRP3O-A.png";

//#endregion
//#region entries/entry.js
console.log(image_default);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entries/entry.js
+++ rolldown	entries_entry.js
@@ -1,2 +1,2 @@
-var image_default = "../image-LSAMBFUD.png";
+var image_default = "assets/image-3GRP3O-A.png";
 console.log(image_default);

```