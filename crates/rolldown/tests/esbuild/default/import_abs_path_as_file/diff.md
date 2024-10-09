# Diff
## /out/entry.js
### esbuild
```js
// Users/user/project/node_modules/pkg/index.js
var pkg_default = 123;

// Users/user/project/entry.js
console.log(pkg_default);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var pkg_default = 123;
-console.log(pkg_default);

```