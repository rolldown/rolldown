# Reason
1. different chunk naming style
# Diff
## /out/entry1.js
### esbuild
```js
// Users/user/project/src/entry1.js
import("./package-ZBNDRRRB.js");
```
### rolldown
```js

//#region src/entry1.js
import("./package_index.js");

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry1.js
+++ rolldown	src_entry1.js
@@ -1,1 +1,1 @@
-import("./package-ZBNDRRRB.js");
+import("./package_index.js");

```
## /out/entry2.js
### esbuild
```js
// Users/user/project/src/entry2.js
import("./package-ZBNDRRRB.js");
```
### rolldown
```js

//#region src/entry2.js
import("./package_index.js");

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry2.js
+++ rolldown	src_entry2.js
@@ -1,1 +1,1 @@
-import("./package-ZBNDRRRB.js");
+import("./package_index.js");

```