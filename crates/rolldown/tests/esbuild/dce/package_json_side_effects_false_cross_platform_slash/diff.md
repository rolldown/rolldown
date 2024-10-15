# Reason
1. side effects detect
# Diff
## /out.js
### esbuild
```js
// Users/user/project/node_modules/demo-pkg/foo.js
console.log("foo");

// Users/user/project/node_modules/demo-pkg/bar/index.js
console.log("bar");
```
### rolldown
```js

//#region node_modules/demo-pkg/foo.js
console.log("foo");

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	src_entry.js
@@ -1,2 +1,1 @@
 console.log("foo");
-console.log("bar");

```