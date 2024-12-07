# Reason
1. resolve `mts` in ts
# Diff
## /out.js
### esbuild
```js
// imported.mts
console.log("works");
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,1 +0,0 @@
-console.log("works");

```