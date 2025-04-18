# Reason
1. `b` has `sideEffects: false`, so the `throw` should be stripped, same as webpack
# Diff
## /out.js
### esbuild
```js
// Users/user/project/node_modules/d/index.js
var foo = 123;

// Users/user/project/node_modules/b/index.js
throw "keep this";

// Users/user/project/src/entry.js
console.log(foo);
```
### rolldown
```js
//#region node_modules/d/index.js
const foo = 123;

//#endregion
//#region src/entry.js
console.log(foo);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	src_entry.js
@@ -1,3 +1,2 @@
 var foo = 123;
-throw "keep this";
 console.log(foo);

```