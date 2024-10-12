# Diff
## /out.js
### esbuild
```js
// inject.js
console.log("injected");
```
### rolldown
```js


```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,1 +0,0 @@
-console.log("injected");

```