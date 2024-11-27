# Reason
1. different deconflict naming style and order
# Diff
## /out.js
### esbuild
```js
// foo/test.js
var test_exports = {};
__export(test_exports, {
  foo: () => foo
});
var foo = 123;

// bar/test.js
var test_exports2 = {};
__export(test_exports2, {
  bar: () => bar
});
var bar = 123;

// entry.js
console.log(exports, module.exports, test_exports, test_exports2);
```
### rolldown
```js

const node_assert = __toESM(require("node:assert"));

//#region foo/test.js
let foo = 123;
var test_exports$1 = {};
__export(test_exports$1, { foo: () => foo });

//#endregion
//#region bar/test.js
let bar = 123;
var test_exports = {};
__export(test_exports, { bar: () => bar });

//#endregion
//#region entry.js
console.log(exports, module.exports);
node_assert.default.deepEqual(test_exports$1, { foo: 123 });
node_assert.default.deepEqual(test_exports, { bar: 123 });

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,11 +1,18 @@
-var test_exports = {};
-__export(test_exports, {
+var node_assert = __toESM(require("node:assert"));
+var foo = 123;
+var test_exports$1 = {};
+__export(test_exports$1, {
     foo: () => foo
 });
-var foo = 123;
-var test_exports2 = {};
-__export(test_exports2, {
+var bar = 123;
+var test_exports = {};
+__export(test_exports, {
     bar: () => bar
 });
-var bar = 123;
-console.log(exports, module.exports, test_exports, test_exports2);
+console.log(exports, module.exports);
+node_assert.default.deepEqual(test_exports$1, {
+    foo: 123
+});
+node_assert.default.deepEqual(test_exports, {
+    bar: 123
+});

```