# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/src/node_modules/pkg/a.js
console.log("a.js");

// Users/user/project/src/node_modules/pkg/b.js
console.log("b.js");

// Users/user/project/src/node_modules/pkg/some-star/c.js
console.log("c.js");

// Users/user/project/src/node_modules/pkg/some-slash/d.js
console.log("d.js");
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	
@@ -1,4 +0,0 @@
-console.log("a.js");
-console.log("b.js");
-console.log("c.js");
-console.log("d.js");

```