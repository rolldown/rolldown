# Diff
## /out.js
### esbuild
```js
// Users/user/project/src/dir/index.js
var dir_default = 123;

// Users/user/project/src/entry.js
console.log(dir_default);
```
### rolldown
```js
import { default as assert } from "node:assert";

//#region dir/index.js
var dir_index_default = 123;

//#endregion
//#region entry.js
assert.equal(dir_index_default, 123);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,2 +1,2 @@
-var dir_default = 123;
-console.log(dir_default);
+var dir_index_default = 123;
+console.log(dir_index_default);

```