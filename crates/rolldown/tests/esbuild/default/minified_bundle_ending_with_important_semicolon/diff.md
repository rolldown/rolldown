# Diff
## /out.js
### esbuild
```js
(()=>{while(foo());})();
```
### rolldown
```js

//#region entry.js
while (foo());

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,3 +1,1 @@
-(() => {
-    while (foo()) ;
-})();
+while (foo()) ;

```