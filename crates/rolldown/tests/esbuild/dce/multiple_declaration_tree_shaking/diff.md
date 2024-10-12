# Diff
## /out/var2.js
### esbuild
```js
// var2.js
var x = 1;
console.log(x);
var x = 2;
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/var2.js
+++ rolldown	
@@ -1,3 +0,0 @@
-var x = 1;
-console.log(x);
-var x = 2;

```
## /out/var3.js
### esbuild
```js
// var3.js
var x = 1;
console.log(x);
var x = 2;
console.log(x);
var x = 3;
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/var3.js
+++ rolldown	
@@ -1,5 +0,0 @@
-var x = 1;
-console.log(x);
-var x = 2;
-console.log(x);
-var x = 3;

```