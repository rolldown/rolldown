# Diff
## /out.js
### esbuild
```js
(() => {
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
@@ -1,1 +1,1 @@
-(() => {})();
+var hasBar = typeof bar !== "undefined";

```