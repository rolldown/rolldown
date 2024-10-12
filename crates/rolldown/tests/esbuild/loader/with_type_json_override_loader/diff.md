# Diff
## entry.js
### esbuild
```js
// foo.js
var foo_default = { "this is json not js": true };

// entry.js
console.log(foo_default);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	entry.js
+++ rolldown	
@@ -1,4 +0,0 @@
-var foo_default = {
-    "this is json not js": true
-};
-console.log(foo_default);

```