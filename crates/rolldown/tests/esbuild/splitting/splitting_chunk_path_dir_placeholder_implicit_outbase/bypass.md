# Reason
1. different file system
# Diff
## /out/entry.js
### esbuild
```js
// project/entry.js
console.log(import("./output-path/should-contain/this-text/file-G2XPANW2.js"));
```
### rolldown
```js

//#region entry.js
console.log(import("./file.js"));

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,1 +1,1 @@
-console.log(import("./output-path/should-contain/this-text/file-G2XPANW2.js"));
+console.log(import("./file.js"));

```