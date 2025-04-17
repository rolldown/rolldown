# Reason
1. not support asset path template
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
var image_default = "https://example.com/images/image-LSAMBFUD.png";

// src/entries/entry.js
console.log(image_default);
```
### rolldown
```js

//#region images/image.png
var image_default = "assets/image-Dq1zDy-k.png";

//#region entries/entry.js
console.log(image_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/entries/entry.js
+++ rolldown	entries_entry.js
@@ -1,2 +1,2 @@
-var image_default = "https://example.com/images/image-LSAMBFUD.png";
+var image_default = "assets/image-Dq1zDy-k.png";
 console.log(image_default);

```