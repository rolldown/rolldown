# Reason
1. rolldown keep unsupported `import.meta` as it is in cjs format.
2. rolldown polyfill `import.meta.url` with `require("url").pathToFileURL(__filename).href` in cjs format and node platform.
# Diff
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
+console.log(require("url").pathToFileURL(__filename).href, {}.path);

```