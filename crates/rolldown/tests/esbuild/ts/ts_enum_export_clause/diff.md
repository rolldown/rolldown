# Diff
## /out/entry.js
### esbuild
```js
// entry.ts
console.log([
  1 /* A */,
  2 /* B */,
  3 /* C */,
  4 /* D */
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
-console.log([1, 2, 3, 4]);

```