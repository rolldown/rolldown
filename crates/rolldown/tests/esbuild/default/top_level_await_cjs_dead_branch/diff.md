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


```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,2 +0,0 @@
-if (false) foo;
-if (false) for (foo of bar) ;

```