# Reason
1. `oxc` inject align with `@rollup/plugin-inject` don't support inject files directly
# Diff
## /out.js
### esbuild
```js
console.log("must be present");
console.log("here");
```
### rolldown
```js

//#region entry.ts
console.log("here");
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,2 +1,1 @@
-console.log("must be present");
 console.log("here");

```