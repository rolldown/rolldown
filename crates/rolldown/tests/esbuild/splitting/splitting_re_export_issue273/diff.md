## /out/a.js
### esbuild
```js
import {
  a
} from "./chunk-RLFZNZQZ.js";
export {
  a
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
-import {a} from "./chunk-RLFZNZQZ.js";
-export {a};

```
## /out/b.js
### esbuild
```js
import {
  a
} from "./chunk-RLFZNZQZ.js";
export {
  a
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
-import {a} from "./chunk-RLFZNZQZ.js";
-export {a};

```
## /out/chunk-RLFZNZQZ.js
### esbuild
```js
// a.js
var a = 1;

export {
  a
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-RLFZNZQZ.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var a = 1;
-export {a};

```
# Diff
## /out/a.js
### esbuild
```js
import {
  a
} from "./chunk-RLFZNZQZ.js";
export {
  a
};
```
### rolldown
```js
import { a } from "./a2.js";

export { a };
```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	a.js
@@ -1,2 +1,2 @@
-import {a} from "./chunk-RLFZNZQZ.js";
+import {a} from "./a2.js";
 export {a};

```
## /out/b.js
### esbuild
```js
import {
  a
} from "./chunk-RLFZNZQZ.js";
export {
  a
};
```
### rolldown
```js
import { a } from "./a2.js";

export { a };
```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	b.js
@@ -1,2 +1,2 @@
-import {a} from "./chunk-RLFZNZQZ.js";
+import {a} from "./a2.js";
 export {a};

```
## /out/chunk-RLFZNZQZ.js
### esbuild
```js
// a.js
var a = 1;

export {
  a
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-RLFZNZQZ.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var a = 1;
-export {a};

```