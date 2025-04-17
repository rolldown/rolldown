# Reason
1. different fs
2. different naming style
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
import assert from "node:assert";

//#region index.js
var require_parent_dir_es6_default = 123;

//#region dir/entry.js
assert.equal(require_parent_dir_es6_default, 123);

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	dir_entry.js
@@ -1,2 +1,2 @@
-var src_default = 123;
-console.log(src_default);
+var require_parent_dir_es6_default = 123;
+console.log(require_parent_dir_es6_default);

```