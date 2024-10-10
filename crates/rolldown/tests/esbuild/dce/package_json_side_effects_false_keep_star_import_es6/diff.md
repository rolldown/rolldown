<<<<<<< HEAD
# Diff
## /out.js
### esbuild
```js
// Users/user/project/node_modules/demo-pkg/index.js
var demo_pkg_exports = {};
__export(demo_pkg_exports, {
  foo: () => foo
});
var foo = 123;
console.log("hello");

// Users/user/project/src/entry.js
console.log(demo_pkg_exports);
```
### rolldown
```js
import { default as assert } from "node:assert";


//#region node_modules/demo-pkg/index.js
var demo_pkg_index_exports = {};
__export(demo_pkg_index_exports, { foo: () => foo });
const foo = 123;
console.log("hello");

//#endregion
//#region src/entry.js
assert.deepEqual(demo_pkg_index_exports, { foo: 123 });

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	src_entry_js.js
@@ -1,7 +1,7 @@
-var demo_pkg_exports = {};
-__export(demo_pkg_exports, {
+var demo_pkg_index_exports = {};
+__export(demo_pkg_index_exports, {
     foo: () => foo
 });
 var foo = 123;
 console.log("hello");
-console.log(demo_pkg_exports);
+console.log(demo_pkg_index_exports);

```