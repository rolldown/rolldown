# Reason
1. there should not exist empty chunk
2. import('./entry.js') should be rewrite to `require_entry`
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

//#region entry.js
import("./a.js");
import("./b.js");
import("./c.js");
import("./entry.js");
await 0;
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,5 +1,5 @@
-import("./a-3BAWOBN3.js");
-import("./b-2IGVSUS7.js");
-import("./c-DMBKURS2.js");
-require_entry();
+import("./a.js");
+import("./b.js");
+import("./c.js");
+import("./entry.js");
 await 0;

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