# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/node_modules/demo-pkg/b.js
var b_default = "b";

// Users/user/project/src/entry.js
console.log(b_default);
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
-var b_default = "b";
-console.log(b_default);

```