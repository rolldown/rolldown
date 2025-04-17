# Reason
1. different chunk naming style
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

//#region b.js
var q = 6;

//#region a.js
var p = 5;

export { p, q };
```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-HK23737J.js
+++ rolldown	a2.js
@@ -1,3 +1,3 @@
-var p = 5;
 var q = 6;
-export {q, p};
+var p = 5;
+export {p, q};

```