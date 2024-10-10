## /out/a.js
### esbuild
```js
import {
  foo,
  setFoo
} from "./chunk-GX7G2SBE.js";

// a.js
setFoo(123);
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
@@ -1,3 +0,0 @@
-import {foo, setFoo} from "./chunk-GX7G2SBE.js";
-setFoo(123);
-console.log(foo);

```
## /out/b.js
### esbuild
```js
import {
  foo
} from "./chunk-GX7G2SBE.js";

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
-import {foo} from "./chunk-GX7G2SBE.js";
-console.log(foo);

```
## /out/chunk-GX7G2SBE.js
### esbuild
```js
// shared.js
var foo;
function setFoo(value) {
  foo = value;
}

export {
  foo,
  setFoo
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-GX7G2SBE.js
+++ rolldown	
@@ -1,5 +0,0 @@
-var foo;
-function setFoo(value) {
-    foo = value;
-}
-export {foo, setFoo};

```
# Diff
## /out/a.js
### esbuild
```js
import {
  foo,
  setFoo
} from "./chunk-GX7G2SBE.js";

// a.js
setFoo(123);
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
@@ -1,3 +0,0 @@
-import {foo, setFoo} from "./chunk-GX7G2SBE.js";
-setFoo(123);
-console.log(foo);

```
## /out/b.js
### esbuild
```js
import {
  foo
} from "./chunk-GX7G2SBE.js";

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
-import {foo} from "./chunk-GX7G2SBE.js";
-console.log(foo);

```
## /out/chunk-GX7G2SBE.js
### esbuild
```js
// shared.js
var foo;
function setFoo(value) {
  foo = value;
}

export {
  foo,
  setFoo
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-GX7G2SBE.js
+++ rolldown	
@@ -1,5 +0,0 @@
-var foo;
-function setFoo(value) {
-    foo = value;
-}
-export {foo, setFoo};

```