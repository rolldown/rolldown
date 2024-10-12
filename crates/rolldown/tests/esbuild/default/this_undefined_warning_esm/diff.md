# Diff
## /out/entry.js
### esbuild
```js
// file1.js
var file1_default = [void 0, void 0];

// node_modules/pkg/file2.js
var file2_default = [void 0, void 0];

// entry.js
console.log(file1_default, file2_default);
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
-var file1_default = [void 0, void 0];
-var file2_default = [void 0, void 0];
-console.log(file1_default, file2_default);

```