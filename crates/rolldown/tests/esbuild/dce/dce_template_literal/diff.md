# Diff
## /out/entry.js
### esbuild
```js
// entry.js
var alsoKeep;
var a = `${keep}`;
var c = `${keep ? 1 : 2n}`;
var e = `${alsoKeep}`;
```
### rolldown
```js
//#region entry.js
var alsoKeep;
`${keep}`;
`${keep ? 1 : 2n}`;
`${alsoKeep}`;

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,4 +1,4 @@
 var alsoKeep;
-var a = `${keep}`;
-var c = `${keep ? 1 : 2n}`;
-var e = `${alsoKeep}`;
+`${keep}`;
+`${keep ? 1 : 2n}`;
+`${alsoKeep}`;

```