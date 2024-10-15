# Reason
1. don't rewrite top level binding
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
let x = function foo(foo) {
	var foo;
	return foo;
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
+var x = function foo(foo) {
     var foo;
     return foo;
 };

```