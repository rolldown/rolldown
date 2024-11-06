# Reason
1. this is expected, since we don't support `convertMode`
2. the diff is because oxc eliminated the dead branch
# Diff
## /out.js
### esbuild
```js
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