# Diff
## /out/bytesInOutput should be at least 99 (1).js
### esbuild
```js
// project/111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111.file
var __default = "./111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111-55DNWN2R.file";

// project/bytesInOutput should be at least 99 (1).js
console.log(__default);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/bytesInOutput should be at least 99 (1).js
+++ rolldown	
@@ -1,2 +0,0 @@
-var __default = "./111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111-55DNWN2R.file";
-console.log(__default);

```
## /out/bytesInOutput should be at least 99 (2).js
### esbuild
```js
// project/bytesInOutput should be at least 99 (2).js
import a from "./222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222-55DNWN2R.copy";
console.log(a);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/bytesInOutput should be at least 99 (2).js
+++ rolldown	
@@ -1,2 +0,0 @@
-import a from "./222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222-55DNWN2R.copy";
-console.log(a);

```
## /out/bytesInOutput should be at least 99 (3).js
### esbuild
```js
// project/bytesInOutput should be at least 99 (3).js
import("./333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333-DH3FVEAA.js").then(console.log);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/bytesInOutput should be at least 99 (3).js
+++ rolldown	
@@ -1,1 +0,0 @@
-import("./333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333-DH3FVEAA.js").then(console.log);

```