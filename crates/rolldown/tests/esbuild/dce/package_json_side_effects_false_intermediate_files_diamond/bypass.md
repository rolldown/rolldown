# Reason
1. `b1` and `b2` has `sideEffects: false`, so the `throw` should be stripped, same as webpack
# Diff
## /out.js
### esbuild
```js
// Users/user/project/node_modules/d/index.js
var foo = 123;

// Users/user/project/node_modules/b1/index.js
throw "keep this 1";

// Users/user/project/node_modules/b2/index.js
throw "keep this 2";

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
@@ -1,4 +1,2 @@
 var foo = 123;
-throw "keep this 1";
-throw "keep this 2";
 console.log(foo);

```