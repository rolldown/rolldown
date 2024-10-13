# Reason
1. not support copy loader
# Diff
## /out/src/entry.js
### esbuild
```js
(() => {
  console.log("entry");
})();
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/src/entry.js
+++ rolldown	
@@ -1,3 +0,0 @@
-(() => {
-    console.log("entry");
-})();

```
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