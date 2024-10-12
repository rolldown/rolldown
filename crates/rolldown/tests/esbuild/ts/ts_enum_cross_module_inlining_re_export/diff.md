# Diff
## /out/entry.js
### esbuild
```js
// entry.js
console.log([
  "a" /* x */,
  "b" /* x */,
  "c" /* x */
]);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,1 +0,0 @@
-console.log(["a", "b", "c"]);

```