# Diff
## /out/entry.js
### esbuild
```js
// entry.js
import("./foo-R2VCCZUR.js").then(({ bar }) => console.log(bar));
```
### rolldown
```js
//#region entry.js
import("./foo.js").then(({ bar }) => console.log(bar));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,1 +1,1 @@
-import("./foo-R2VCCZUR.js").then(({bar}) => console.log(bar));
+import("./foo.js").then(({bar}) => console.log(bar));

```