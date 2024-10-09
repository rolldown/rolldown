# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/src/foo-import.js
var foo_import_default = "foo";

// Users/user/project/src/index.js
var src_default = "index";
console.log(src_default, foo_import_default);
export {
  src_default as default
};
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
-var foo_import_default = "foo";
-var src_default = "index";
-console.log(src_default, foo_import_default);
-export {src_default as default};

```