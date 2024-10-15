# Diff
## /out.js
### esbuild
```js
// foo.ts
var foo_exports = {};

// entry.js
console.log(foo_exports);
```
### rolldown
```js


//#region foo.ts
var foo_exports = {};
__export(foo_exports, { nope: () => nope });

//#endregion
//#region entry.js
console.log(foo_exports);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,2 +1,5 @@
 var foo_exports = {};
+__export(foo_exports, {
+    nope: () => nope
+});
 console.log(foo_exports);

```