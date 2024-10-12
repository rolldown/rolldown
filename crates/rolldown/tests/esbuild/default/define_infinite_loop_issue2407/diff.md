# Diff
## /out.js
### esbuild
```js
// entry.js
b.c();
y();
```
### rolldown
```js

//#region entry.js
a.b();
x.y();

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,2 +1,2 @@
-b.c();
-y();
+a.b();
+x.y();

```