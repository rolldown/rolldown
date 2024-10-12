# Diff
## /out/common-LSAMBFUD.png
### esbuild
```js
x
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/common-LSAMBFUD.png
+++ rolldown	
@@ -1,1 +0,0 @@
-x;

```
## /out/entries/entry.js
### esbuild
```js
// src/shared/common.png
var common_default = "../common-LSAMBFUD.png";

// src/shared/common.js
console.log(common_default);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entries/entry.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var common_default = "../common-LSAMBFUD.png";
-console.log(common_default);

```
## /out/entries/other/entry.js
### esbuild
```js
// src/shared/common.png
var common_default = "../../common-LSAMBFUD.png";

// src/shared/common.js
console.log(common_default);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entries/other/entry.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var common_default = "../../common-LSAMBFUD.png";
-console.log(common_default);

```