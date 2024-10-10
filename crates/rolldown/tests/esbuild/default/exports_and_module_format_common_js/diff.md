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

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,11 +0,0 @@
-var test_exports = {};
-__export(test_exports, {
-    foo: () => foo
-});
-var foo = 123;
-var test_exports2 = {};
-__export(test_exports2, {
-    bar: () => bar
-});
-var bar = 123;
-console.log(exports, module.exports, test_exports, test_exports2);

```