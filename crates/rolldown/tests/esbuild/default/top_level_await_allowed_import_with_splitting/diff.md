# Diff
## /out/entry.js
### esbuild
```js
// entry.js
import("./a-3BAWOBN3.js");
import("./b-2IGVSUS7.js");
import("./c-DMBKURS2.js");
require_entry();
await 0;
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,5 +0,0 @@
-import("./a-3BAWOBN3.js");
-import("./b-2IGVSUS7.js");
-import("./c-DMBKURS2.js");
-require_entry();
-await 0;

```
## /out/c-DMBKURS2.js
### esbuild
```js
import "./chunk-GETF6B5C.js";
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/c-DMBKURS2.js
+++ rolldown	
@@ -1,1 +0,0 @@
-import "./chunk-GETF6B5C.js";

```
## /out/b-2IGVSUS7.js
### esbuild
```js
import "./chunk-QJYGFXJG.js";
import "./chunk-GETF6B5C.js";
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/b-2IGVSUS7.js
+++ rolldown	
@@ -1,2 +0,0 @@
-import "./chunk-QJYGFXJG.js";
-import "./chunk-GETF6B5C.js";

```
## /out/a-3BAWOBN3.js
### esbuild
```js
import "./chunk-QJYGFXJG.js";
import "./chunk-GETF6B5C.js";
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/a-3BAWOBN3.js
+++ rolldown	
@@ -1,2 +0,0 @@
-import "./chunk-QJYGFXJG.js";
-import "./chunk-GETF6B5C.js";

```
## /out/chunk-GETF6B5C.js
### esbuild
```js
// c.js
await 0;
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-GETF6B5C.js
+++ rolldown	
@@ -1,1 +0,0 @@
-await 0;

```