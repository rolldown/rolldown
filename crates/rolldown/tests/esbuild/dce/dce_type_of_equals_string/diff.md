# Diff
## /out.js
### esbuild
```js
(() => {
  // entry.js
  if (false) console.log(hasBar);
})();
```
### rolldown
```js

//#region entry.js
var hasBar = typeof bar !== "undefined";

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,3 +1,1 @@
-(() => {
-    if (false) console.log(hasBar);
-})();
+var hasBar = typeof bar !== "undefined";

```