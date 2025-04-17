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
//#region rolldown:runtime
var __defProp = Object.defineProperty;
var __export = (target, all) => {
	for (var name in all) __defProp(target, name, {
		get: all[name],
		enumerable: true
	});
};


//#region foo/test.js
var test_exports$1 = {};
__export(test_exports$1, { foo: () => foo });
let foo = 123;

//#region bar/test.js
var test_exports = {};
__export(test_exports, { bar: () => bar });
let bar = 123;

//#region entry.js
console.log(exports, module.exports, test_exports$1, test_exports);

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,11 +1,18 @@
-var o = {};
-p(o, {
-    foo: () => l
+var __defProp = Object.defineProperty;
+var __export = (target, all) => {
+    for (var name in all) __defProp(target, name, {
+        get: all[name],
+        enumerable: true
+    });
+};
+var test_exports$1 = {};
+__export(test_exports$1, {
+    foo: () => foo
 });
-var l = 123;
-var r = {};
-p(r, {
-    bar: () => m
+var foo = 123;
+var test_exports = {};
+__export(test_exports, {
+    bar: () => bar
 });
-var m = 123;
-console.log(exports, module.exports, o, r);
+var bar = 123;
+console.log(exports, module.exports, test_exports$1, test_exports);

```