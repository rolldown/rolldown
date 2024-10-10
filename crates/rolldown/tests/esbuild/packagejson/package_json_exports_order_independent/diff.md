# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/node_modules/pkg1/2/bar.js
console.log("SUCCESS");

// Users/user/project/node_modules/pkg2/1/bar.js
console.log("SUCCESS");
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	
@@ -1,2 +0,0 @@
-console.log("SUCCESS");
-console.log("SUCCESS");

```