# Diff
## /out/replaced.js
### esbuild
```js
// replaced.js
console.log(1);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/replaced.js
+++ rolldown	
@@ -1,1 +0,0 @@
-console.log(1);

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

```
### diff
```diff
===================================================================
--- esbuild	/out/kept.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var import_meta = {};
-console.log(import_meta.y);

```