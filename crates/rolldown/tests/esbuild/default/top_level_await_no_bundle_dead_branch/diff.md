# Diff
## /out.js
### esbuild
```js
if (false) await foo;
if (false) for await (foo of bar) ;
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
-if (false) await foo;
-if (false) for await (foo of bar) ;

```