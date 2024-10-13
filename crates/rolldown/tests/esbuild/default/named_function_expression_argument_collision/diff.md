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


```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,4 +0,0 @@
-let x = function (foo) {
-    var foo;
-    return foo;
-};

```