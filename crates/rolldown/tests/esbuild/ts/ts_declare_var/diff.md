# Diff
## /out.js
### esbuild
```js
// entry.ts
var foo = bar();
```
### rolldown
```js
//#region entry.ts
bar();

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,1 +1,1 @@
-var foo = bar();
+bar();

```