## /out/a.js
### esbuild
```js
import {
  a
} from "./chunk-Y3CWGI3W.js";

// a.js
console.log(a);
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
-import {a} from "./chunk-Y3CWGI3W.js";
-console.log(a);

```
## /out/b.js
### esbuild
```js
import {
  b
} from "./chunk-Y3CWGI3W.js";

// b.js
console.log(b);
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
-import {b} from "./chunk-Y3CWGI3W.js";
-console.log(b);

```
## /out/chunk-Y3CWGI3W.js
### esbuild
```js
// shared.js
var a = 1;
var b = 2;
console.log("side effect");

export {
  a,
  b
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-Y3CWGI3W.js
+++ rolldown	
@@ -1,4 +0,0 @@
-var a = 1;
-var b = 2;
-console.log("side effect");
-export {a, b};

```
# Diff
## /out/a.js
### esbuild
```js
import {
  a
} from "./chunk-Y3CWGI3W.js";

// a.js
console.log(a);
```
### rolldown
```js
import { a } from "./shared.js";

//#region a.js
console.log(a);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	a.js
@@ -1,2 +1,2 @@
-import {a} from "./chunk-Y3CWGI3W.js";
+import {a} from "./shared.js";
 console.log(a);

```
## /out/b.js
### esbuild
```js
import {
  b
} from "./chunk-Y3CWGI3W.js";

// b.js
console.log(b);
```
### rolldown
```js
import { b } from "./shared.js";

//#region b.js
console.log(b);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	b.js
@@ -1,2 +1,2 @@
-import {b} from "./chunk-Y3CWGI3W.js";
+import {b} from "./shared.js";
 console.log(b);

```
## /out/chunk-Y3CWGI3W.js
### esbuild
```js
// shared.js
var a = 1;
var b = 2;
console.log("side effect");

export {
  a,
  b
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-Y3CWGI3W.js
+++ rolldown	
@@ -1,4 +0,0 @@
-var a = 1;
-var b = 2;
-console.log("side effect");
-export {a, b};

```