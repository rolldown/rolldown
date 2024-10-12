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

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,4 +0,0 @@
-var alsoKeep;
-var a = `${keep}`;
-var c = `${keep ? 1 : 2n}`;
-var e = `${alsoKeep}`;

```