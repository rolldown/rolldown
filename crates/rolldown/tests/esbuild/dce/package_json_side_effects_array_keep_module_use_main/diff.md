# Reason
1. side effects detect not align
# Diff
## /out.js
### esbuild
```js
// Users/user/project/src/entry.js
console.log("unused import");
```
### rolldown
```js

//#region node_modules/demo-pkg/index-module.js
console.log("TEST FAILED");
//#endregion

//#region src/entry.js
console.log("unused import");
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	src_entry.js
@@ -1,1 +1,2 @@
+console.log("TEST FAILED");
 console.log("unused import");

```