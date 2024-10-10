# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/node_modules/pkg/dir/baz-foo.js
console.log("works");

// Users/user/project/node_modules/pkg2/public/abc.js
console.log("abc");

// Users/user/project/node_modules/pkg2/public/xyz.js
console.log("xyz");
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	
@@ -1,3 +0,0 @@
-console.log("works");
-console.log("abc");
-console.log("xyz");

```