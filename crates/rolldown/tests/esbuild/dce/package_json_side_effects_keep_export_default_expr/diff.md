# Diff
## /out.js
### esbuild
```js
// Users/user/project/node_modules/demo-pkg/index.js
var demo_pkg_default = exprWithSideEffects();

// Users/user/project/src/entry.js
console.log(demo_pkg_default);
```
### rolldown
```js

//#region node_modules/demo-pkg/index.js
var demo_pkg_index_default = exprWithSideEffects();

//#endregion
//#region src/entry.js
console.log(demo_pkg_index_default);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	src_entry.js
@@ -1,2 +1,2 @@
-var demo_pkg_default = exprWithSideEffects();
-console.log(demo_pkg_default);
+var demo_pkg_index_default = exprWithSideEffects();
+console.log(demo_pkg_index_default);

```