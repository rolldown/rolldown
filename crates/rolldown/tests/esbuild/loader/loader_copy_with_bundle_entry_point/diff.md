# Diff
## /out/assets/some.file
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
--- esbuild	/out/assets/some.file
+++ rolldown	
@@ -1,1 +0,0 @@
-stuff;

```
## /out/src/entry.js
### esbuild
```js
// Users/user/project/src/entry.js
import x from "../assets/some.file";
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
-import x from "../assets/some.file";
-console.log(x);

```