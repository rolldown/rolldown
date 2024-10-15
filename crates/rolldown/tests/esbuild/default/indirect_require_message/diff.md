# Reason
1. not align
# Diff
## /out/array.js
### esbuild
```js

```
### rolldown
```js

//#region array.js
let x = [require];

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/array.js
+++ rolldown	array.js
@@ -0,0 +1,1 @@
+var x = [require];

```
## /out/assign.js
### esbuild
```js
// assign.js
__require = x;
```
### rolldown
```js

//#region assign.js
require = x;

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/assign.js
+++ rolldown	assign.js
@@ -1,1 +1,1 @@
-__require = x;
+require = x;

```
## /out/dot.js
### esbuild
```js
// dot.js
var x = __require.cache;
```
### rolldown
```js

//#region dot.js
let x = require.cache;

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/dot.js
+++ rolldown	dot.js
@@ -1,1 +1,1 @@
-var x = __require.cache;
+var x = require.cache;

```
## /out/ident.js
### esbuild
```js

```
### rolldown
```js

//#region ident.js
let x = require;

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/ident.js
+++ rolldown	ident.js
@@ -0,0 +1,1 @@
+var x = require;

```
## /out/index.js
### esbuild
```js
// index.js
var x = __require[cache];
```
### rolldown
```js

//#region index.js
let x = require[cache];

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/index.js
+++ rolldown	index.js
@@ -1,1 +1,1 @@
-var x = __require[cache];
+var x = require[cache];

```