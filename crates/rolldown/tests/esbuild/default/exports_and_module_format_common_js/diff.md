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

const { default: assert } = __toESM(require("node:assert"));

//#region foo/test.js
var test_exports$1 = {};
__export(test_exports$1, { foo: () => foo });
let foo = 123;

//#endregion
//#region bar/test.js
var test_exports = {};
__export(test_exports, { bar: () => bar });
let bar = 123;

//#endregion
//#region entry.js
assert.deepEqual(test_exports$1, { foo: 123 });
assert.deepEqual(test_exports, { bar: 123 });
console.log(exports, module.exports);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,11 +1,14 @@
-var test_exports = {};
-__export(test_exports, {
+var {default: assert} = __toESM(require("node:assert"));
+var test_exports$1 = {};
+__export(test_exports$1, {
     foo: () => foo
 });
 var foo = 123;
-var test_exports2 = {};
-__export(test_exports2, {
+var test_exports = {};
+__export(test_exports, {
     bar: () => bar
 });
 var bar = 123;
-console.log(exports, module.exports, test_exports, test_exports2);
+console.log(test_exports$1, test_exports);
+console.log(exports, module.exports);
+console.log(test_exports$1, test_exports);

```