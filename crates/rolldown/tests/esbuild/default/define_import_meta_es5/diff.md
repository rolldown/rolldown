# Diff
## /out/replaced.js
### esbuild
```js
// replaced.js
console.log(1);
```
### rolldown
```js

//#region replaced.js
console.log(import.meta.x);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/replaced.js
+++ rolldown	replaced.js
@@ -1,1 +1,1 @@
-console.log(1);
+console.log(import.meta.x);

```
## /out/kept.js
### esbuild
```js
// kept.js
var import_meta = {};
console.log(import_meta.y);
```
### rolldown
```js

//#region kept.js
console.log(import.meta.y);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/kept.js
+++ rolldown	kept.js
@@ -1,2 +1,1 @@
-var import_meta = {};
-console.log(import_meta.y);
+console.log(import.meta.y);

```