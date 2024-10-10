## /out/a.js
### esbuild
```js
import {
  foo
} from "./chunk-25TWIR6T.js";

// a.js
console.log(foo);
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
-import {foo} from "./chunk-25TWIR6T.js";
-console.log(foo);

```
## /out/b.js
### esbuild
```js
import {
  foo
} from "./chunk-25TWIR6T.js";

// b.js
console.log(foo);
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
-import {foo} from "./chunk-25TWIR6T.js";
-console.log(foo);

```
## /out/chunk-25TWIR6T.js
### esbuild
```js
// shared.js
var foo = 123;

export {
  foo
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-25TWIR6T.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var foo = 123;
-export {foo};

```
# Diff
## /out/a.js
### esbuild
```js
import {
  foo
} from "./chunk-25TWIR6T.js";

// a.js
console.log(foo);
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
-import {foo} from "./chunk-25TWIR6T.js";
-console.log(foo);

```
## /out/b.js
### esbuild
```js
import {
  foo
} from "./chunk-25TWIR6T.js";

// b.js
console.log(foo);
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
-import {foo} from "./chunk-25TWIR6T.js";
-console.log(foo);

```
## /out/chunk-25TWIR6T.js
### esbuild
```js
// shared.js
var foo = 123;

export {
  foo
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-25TWIR6T.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var foo = 123;
-export {foo};

```