# Reason
1. different chunk naming style
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