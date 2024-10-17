# Reason
1. custom diff resolver
# Diff
## /out/entry.notjs
### esbuild
```js
// entry.js
console.log("test");
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.notjs
+++ rolldown	
@@ -1,1 +0,0 @@
-console.log("test");

```