# Reason
1. Not support file loader
# Diff
## /out/images/image-LSAMBFUD.png
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
--- esbuild	/out/images/image-LSAMBFUD.png
+++ rolldown	
@@ -1,1 +0,0 @@
-x;

```
## /out/entries/entry.js
### esbuild
```js
// src/images/image.png
var image_default = "../images/image-LSAMBFUD.png";

// src/entries/entry.js
console.log(image_default);
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
-var image_default = "../images/image-LSAMBFUD.png";
-console.log(image_default);

```