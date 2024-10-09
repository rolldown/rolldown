# Diff
## /out/assign.js
### esbuild
```js
// assign.js
__require = x;
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/assign.js
+++ rolldown	
@@ -1,1 +0,0 @@
-__require = x;

```
## /out/dot.js
### esbuild
```js
// dot.js
var x = __require.cache;
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/dot.js
+++ rolldown	
@@ -1,1 +0,0 @@
-var x = __require.cache;

```
## /out/index.js
### esbuild
```js
// index.js
var x = __require[cache];
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/index.js
+++ rolldown	
@@ -1,1 +0,0 @@
-var x = __require[cache];

```