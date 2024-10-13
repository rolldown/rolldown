# Diff
## /out/entry.js
### esbuild
```js
// Users/user/project/node_modules/pkg/index.js
var pkg_default = 123;

// Users/user/project/entry.js
console.log(pkg_default);
```
### rolldown
```js
import { default as pkg } from "C:\Users\user\project\node_modules\pkg";

//#region entry.js
console.log(pkg);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,2 +1,6 @@
-var pkg_default = 123;
-console.log(pkg_default);
+import { default as pkg } from "C:\Users\user\project\node_modules\pkg";
+
+//#region entry.js
+console.log(pkg);
+
+//#endregion

```