# Reason
1. not support copy loader
# Diff
## /out/some-BYATPJRB.file
### esbuild
```js
stuff
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/some-BYATPJRB.file
+++ rolldown	
@@ -1,1 +0,0 @@
-stuff;

```
## /out/src/entry.js
### esbuild
```js
// Users/user/project/src/entry.js
import x from "../some-BYATPJRB.file";
console.log(x);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/src/entry.js
+++ rolldown	
@@ -1,2 +0,0 @@
-import x from "../some-BYATPJRB.file";
-console.log(x);

```