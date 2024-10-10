# Diff
## /out/entry.js
### esbuild
```js
// a.js
console.log("in a");

// b.js
console.log("in b");

// c.js
console.log("in c");
//! Copyright notice 1
//! Copyright notice 2
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,3 +0,0 @@
-console.log("in a");
-console.log("in b");
-console.log("in c");

```