# Reason
1. should not appear top level `await` in cjs
# Diff
## /out.js
### esbuild
```js
// entry.js
if (false) foo;
if (false) for (foo of bar) ;
```
### rolldown
```js

//#region entry.js
if (false) await foo;
if (false) for await (foo of bar);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,2 +1,2 @@
-if (false) foo;
-if (false) for (foo of bar) ;
+if (false) await foo;
+if (false) for await (foo of bar) ;

```