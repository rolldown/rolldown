# Reason
1. trivial rewrite tool diff
# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/src/foo-import.js
var foo_import_default = "foo";

// Users/user/project/src/index.js
var src_default = "index";
console.log(src_default, foo_import_default);
export {
  src_default as default
};
```
### rolldown
```js
import assert from "node:assert";

//#region src/foo-import.js
var foo_import_default = "foo";
//#endregion

//#region src/index.js
var src_default = "index";
assert.equal(src_default, "index");
assert.equal(foo_import_default, "foo");
//#endregion

export { src_default as default };
```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	entry.js
@@ -1,4 +1,4 @@
 var foo_import_default = "foo";
 var src_default = "index";
-console.log(src_default, foo_import_default);
 export {src_default as default};
+console.log(src_default, foo_import_default);

```