# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/node_modules/demo-pkg/module.js
var module_default = "module";

// Users/user/project/src/entry.js
console.log(module_default);
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
-var module_default = "module";
-console.log(module_default);

```