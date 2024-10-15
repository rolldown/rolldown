## /out/a.js
### esbuild
```js
import"./chunk-QPOQRTMB.js";
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
-import "./chunk-QPOQRTMB.js";

```
## /out/b.js
### esbuild
```js
import"./chunk-QPOQRTMB.js";
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	
@@ -1,1 +0,0 @@
-import "./chunk-QPOQRTMB.js";

```
## /out/chunk-QPOQRTMB.js
### esbuild
```js
console.log(123);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-QPOQRTMB.js
+++ rolldown	
@@ -1,1 +0,0 @@
-console.log(123);

```
## /out/c.js
### esbuild
```js
import"./chunk-TOGNOMR3.js";
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/c.js
+++ rolldown	
@@ -1,1 +0,0 @@
-import "./chunk-TOGNOMR3.js";

```
## /out/d.js
### esbuild
```js
import"./chunk-TOGNOMR3.js";
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/d.js
+++ rolldown	
@@ -1,1 +0,0 @@
-import "./chunk-TOGNOMR3.js";

```
## /out/chunk-TOGNOMR3.js
### esbuild
```js
console.log(123);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-TOGNOMR3.js
+++ rolldown	
@@ -1,1 +0,0 @@
-console.log(123);

```
# Diff
## /out/a.js
### esbuild
```js
import"./chunk-QPOQRTMB.js";
```
### rolldown
```js
import "./ab.js";

```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	a.js
@@ -1,1 +1,1 @@
-import "./chunk-QPOQRTMB.js";
+import "./ab.js";

```
## /out/b.js
### esbuild
```js
import"./chunk-QPOQRTMB.js";
```
### rolldown
```js
import "./ab.js";

```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	b.js
@@ -1,1 +1,1 @@
-import "./chunk-QPOQRTMB.js";
+import "./ab.js";

```
## /out/chunk-QPOQRTMB.js
### esbuild
```js
console.log(123);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-QPOQRTMB.js
+++ rolldown	
@@ -1,1 +0,0 @@
-console.log(123);

```
## /out/c.js
### esbuild
```js
import"./chunk-TOGNOMR3.js";
```
### rolldown
```js
import "./cd.js";

```
### diff
```diff
===================================================================
--- esbuild	/out/c.js
+++ rolldown	c.js
@@ -1,1 +1,1 @@
-import "./chunk-TOGNOMR3.js";
+import "./cd.js";

```
## /out/d.js
### esbuild
```js
import"./chunk-TOGNOMR3.js";
```
### rolldown
```js
import "./cd.js";

```
### diff
```diff
===================================================================
--- esbuild	/out/d.js
+++ rolldown	d.js
@@ -1,1 +1,1 @@
-import "./chunk-TOGNOMR3.js";
+import "./cd.js";

```
## /out/chunk-TOGNOMR3.js
### esbuild
```js
console.log(123);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-TOGNOMR3.js
+++ rolldown	
@@ -1,1 +0,0 @@
-console.log(123);

```