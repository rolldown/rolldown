# Reason
1. we generate same output when minify is disabled 
2. could be done in `minifier`
# Diff
## /out.js
### esbuild
```js
// foo/test.js
var o = {};
p(o, {
  foo: () => l
});
var l = 123;

// bar/test.js
var r = {};
p(r, {
  bar: () => m
});
var m = 123;

// entry.js
console.log(exports, module.exports, o, r);
```
### rolldown
```js
// HIDDEN [rolldown:runtime]

//#region foo/test.js
var test_exports = {};
__export(test_exports, { foo: () => foo });
let foo = 123;

//#endregion
//#region bar/test.js
var test_exports$1 = {};
__export(test_exports$1, { bar: () => bar });
let bar = 123;

//#endregion
//#region entry.js
console.log(exports, module.exports, test_exports, test_exports$1);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,11 +1,11 @@
-var o = {};
-p(o, {
-    foo: () => l
+var test_exports = {};
+__export(test_exports, {
+    foo: () => foo
 });
-var l = 123;
-var r = {};
-p(r, {
-    bar: () => m
+var foo = 123;
+var test_exports$1 = {};
+__export(test_exports$1, {
+    bar: () => bar
 });
-var m = 123;
-console.log(exports, module.exports, o, r);
+var bar = 123;
+console.log(exports, module.exports, test_exports, test_exports$1);

```