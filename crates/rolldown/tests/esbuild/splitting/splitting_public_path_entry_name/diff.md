## /out/a.js
### esbuild
```js
// a.js
import("/www/b-AQIID5BE.js");
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	
@@ -1,1 +0,0 @@
-import("/www/b-AQIID5BE.js");

```
## /out/b-AQIID5BE.js
### esbuild
```js
// b.js
console.log("b");
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/b-AQIID5BE.js
+++ rolldown	
@@ -1,1 +0,0 @@
-console.log("b");

```
# Diff
## /out/a.js
### esbuild
```js
// a.js
import("/www/b-AQIID5BE.js");
```
### rolldown
```js

//#region a.js
import("./b.js");

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	a.js
@@ -1,1 +1,1 @@
-import("/www/b-AQIID5BE.js");
+import("./b.js");

```
## /out/b-AQIID5BE.js
### esbuild
```js
// b.js
console.log("b");
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/b-AQIID5BE.js
+++ rolldown	
@@ -1,1 +0,0 @@
-console.log("b");

```