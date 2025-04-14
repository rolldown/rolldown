# Reason
1. the name `foo` binding of function expression can't not referenced anywhere, rolldown have same behavior as esbuild
# Diff
## /out/entry.js
### esbuild
```js
let x = function(foo) {
  var foo;
  return foo;
};
```
### rolldown
```js

//#region entry.js
let x = function foo(foo$1) {
	var foo$1;
	return foo$1;
};

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,4 +1,4 @@
-let x = function (foo) {
-    var foo;
-    return foo;
+let x = function foo(foo$1) {
+    var foo$1;
+    return foo$1;
 };

```