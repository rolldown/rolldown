# Diff
## /out.js
### esbuild
```js
// Users/user/project/node_modules/demo-pkg/index-main.js
console.log("this should be kept");

// Users/user/project/src/entry.js
console.log("unused import");
```
### rolldown
```js

//#region src/entry.js
console.log("unused import");

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	src_entry.js
@@ -1,2 +1,1 @@
-console.log("this should be kept");
 console.log("unused import");

```