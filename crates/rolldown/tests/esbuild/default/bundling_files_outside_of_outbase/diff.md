# Reason
1. needs custom resolver
# Diff
## /out/_.._/_.._/_.._/src/entry.js
### esbuild
```js
// src/entry.js
console.log("test");
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/_.._/_.._/_.._/src/entry.js
+++ rolldown	
@@ -1,1 +0,0 @@
-console.log("test");

```