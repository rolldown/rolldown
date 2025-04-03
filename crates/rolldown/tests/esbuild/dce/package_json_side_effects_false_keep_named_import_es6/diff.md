# Diff
## /out.js
### esbuild
```js
// Users/user/project/node_modules/demo-pkg/index.js
var foo = 123;
console.log("hello");

// Users/user/project/src/entry.js
console.log(foo);
```
### rolldown
```js
import assert from "node:assert";

//#region node_modules/demo-pkg/index.js
const foo$1 = 123;
console.log("hello");

//#endregion
//#region src/entry.js
assert.equal(foo$1, 123);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	src_entry.js
@@ -1,3 +1,3 @@
-var foo = 123;
+var foo$1 = 123;
 console.log("hello");
-console.log(foo);
+console.log(foo$1);

```