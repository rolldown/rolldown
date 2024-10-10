# Diff
## /out.js
### esbuild
```js
// Users/user/project/src/index.js
var src_default = 123;

// Users/user/project/src/dir/entry.js
console.log(src_default);
```
### rolldown
```js
import { default as assert } from "node:assert";

//#region index.js
var require_parent_dir_es6_index_default = 123;

//#endregion
//#region dir/entry.js
assert.equal(require_parent_dir_es6_index_default, 123);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	dir_entry_js.js
@@ -1,2 +1,2 @@
-var src_default = 123;
-console.log(src_default);
+var require_parent_dir_es6_index_default = 123;
+console.log(require_parent_dir_es6_index_default);

```