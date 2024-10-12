# Diff
## /out.js
### esbuild
```js
// entry.js
(() => {
  const req = __require;
  req("./entry");
})();
```
### rolldown
```js

//#region entry.js
(() => {
	const req = require;
	req("./entry");
})();

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,4 +1,4 @@
 (() => {
-    const req = __require;
+    const req = require;
     req("./entry");
 })();

```