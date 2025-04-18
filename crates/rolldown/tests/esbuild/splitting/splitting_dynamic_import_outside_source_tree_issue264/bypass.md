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
import("./package.js");

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry1.js
+++ rolldown	src_entry1.js
@@ -1,1 +1,1 @@
-import("./package-ZBNDRRRB.js");
+import("./package.js");

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
import("./package.js");

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry2.js
+++ rolldown	src_entry2.js
@@ -1,1 +1,1 @@
-import("./package-ZBNDRRRB.js");
+import("./package.js");

```
## /out/package-ZBNDRRRB.js
### esbuild
```js
// Users/user/project/node_modules/package/index.js
console.log("imported");
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/package-ZBNDRRRB.js
+++ rolldown	
@@ -1,1 +0,0 @@
-console.log("imported");

```