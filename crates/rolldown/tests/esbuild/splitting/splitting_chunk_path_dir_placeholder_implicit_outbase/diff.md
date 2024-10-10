## /out/entry.js
### esbuild
```js
// project/entry.js
console.log(import("./output-path/should-contain/this-text/file-G2XPANW2.js"));
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,1 +0,0 @@
-console.log(import("./output-path/should-contain/this-text/file-G2XPANW2.js"));

```
## /out/output-path/should-contain/this-text/file-G2XPANW2.js
### esbuild
```js
// project/output-path/should-contain/this-text/file.js
console.log("file.js");
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/output-path/should-contain/this-text/file-G2XPANW2.js
+++ rolldown	
@@ -1,1 +0,0 @@
-console.log("file.js");

```
# Diff
## /out/entry.js
### esbuild
```js
// project/entry.js
console.log(import("./output-path/should-contain/this-text/file-G2XPANW2.js"));
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,1 +0,0 @@
-console.log(import("./output-path/should-contain/this-text/file-G2XPANW2.js"));

```
## /out/output-path/should-contain/this-text/file-G2XPANW2.js
### esbuild
```js
// project/output-path/should-contain/this-text/file.js
console.log("file.js");
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/output-path/should-contain/this-text/file-G2XPANW2.js
+++ rolldown	
@@ -1,1 +0,0 @@
-console.log("file.js");

```