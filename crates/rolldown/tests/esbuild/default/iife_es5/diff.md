# Diff
## /out.js
### esbuild
```js
(function() {
  // entry.js
  console.log("test");
})();
```
### rolldown
```js

//#region entry.js
console.log("test");

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,3 +1,1 @@
-(function () {
-    console.log("test");
-})();
+console.log("test");

```