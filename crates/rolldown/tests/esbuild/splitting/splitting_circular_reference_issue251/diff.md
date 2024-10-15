## /out/a.js
### esbuild
```js
import {
  p,
  q
} from "./chunk-HK23737J.js";
export {
  p,
  q
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	
@@ -1,2 +0,0 @@
-import {p, q} from "./chunk-HK23737J.js";
-export {p, q};

```
## /out/b.js
### esbuild
```js
import {
  p,
  q
} from "./chunk-HK23737J.js";
export {
  p,
  q
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	
@@ -1,2 +0,0 @@
-import {p, q} from "./chunk-HK23737J.js";
-export {p, q};

```
## /out/chunk-HK23737J.js
### esbuild
```js
// a.js
var p = 5;

// b.js
var q = 6;

export {
  q,
  p
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-HK23737J.js
+++ rolldown	
@@ -1,3 +0,0 @@
-var p = 5;
-var q = 6;
-export {q, p};

```
# Diff
## /out/a.js
### esbuild
```js
import {
  p,
  q
} from "./chunk-HK23737J.js";
export {
  p,
  q
};
```
### rolldown
```js
import { p, q } from "./a2.js";

export { p, q };
```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	a.js
@@ -1,2 +1,2 @@
-import {p, q} from "./chunk-HK23737J.js";
+import {p, q} from "./a2.js";
 export {p, q};

```
## /out/b.js
### esbuild
```js
import {
  p,
  q
} from "./chunk-HK23737J.js";
export {
  p,
  q
};
```
### rolldown
```js
import { p, q } from "./a2.js";

export { p, q };
```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	b.js
@@ -1,2 +1,2 @@
-import {p, q} from "./chunk-HK23737J.js";
+import {p, q} from "./a2.js";
 export {p, q};

```
## /out/chunk-HK23737J.js
### esbuild
```js
// a.js
var p = 5;

// b.js
var q = 6;

export {
  q,
  p
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-HK23737J.js
+++ rolldown	
@@ -1,3 +0,0 @@
-var p = 5;
-var q = 6;
-export {q, p};

```