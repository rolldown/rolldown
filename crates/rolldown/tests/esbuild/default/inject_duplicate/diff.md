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
+++ rolldown	entry_js.mjs
@@ -1,1 +0,0 @@
-console.log("injected");

```
