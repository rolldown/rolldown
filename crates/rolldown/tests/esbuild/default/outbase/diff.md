# Reason
1. custom diff resolver
# Diff
## /out/a/b/c.js
### esbuild
```js
// a/b/c.js
console.log("c");
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/a/b/c.js
+++ rolldown	
@@ -1,1 +0,0 @@
-console.log("c");

```
## /out/a/b/d.js
### esbuild
```js
// a/b/d.js
console.log("d");
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/a/b/d.js
+++ rolldown	
@@ -1,1 +0,0 @@
-console.log("d");

```