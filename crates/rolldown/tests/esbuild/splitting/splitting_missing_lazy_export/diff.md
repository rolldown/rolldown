## /out/a.js
### esbuild
```js
import {
  foo
} from "./chunk-QVTGQSXT.js";

// a.js
console.log(foo());
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
-import {foo} from "./chunk-QVTGQSXT.js";
-console.log(foo());

```
## /out/b.js
### esbuild
```js
import {
  bar
} from "./chunk-QVTGQSXT.js";

// b.js
console.log(bar());
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
-import {bar} from "./chunk-QVTGQSXT.js";
-console.log(bar());

```
## /out/chunk-QVTGQSXT.js
### esbuild
```js
// empty.js
var empty_exports = {};

// common.js
function foo() {
  return [empty_exports, void 0];
}
function bar() {
  return [void 0];
}

export {
  foo,
  bar
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-QVTGQSXT.js
+++ rolldown	
@@ -1,8 +0,0 @@
-var empty_exports = {};
-function foo() {
-    return [empty_exports, void 0];
-}
-function bar() {
-    return [void 0];
-}
-export {foo, bar};

```
# Diff
## /out/a.js
### esbuild
```js
import {
  foo
} from "./chunk-QVTGQSXT.js";

// a.js
console.log(foo());
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
-import {foo} from "./chunk-QVTGQSXT.js";
-console.log(foo());

```
## /out/b.js
### esbuild
```js
import {
  bar
} from "./chunk-QVTGQSXT.js";

// b.js
console.log(bar());
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
-import {bar} from "./chunk-QVTGQSXT.js";
-console.log(bar());

```
## /out/chunk-QVTGQSXT.js
### esbuild
```js
// empty.js
var empty_exports = {};

// common.js
function foo() {
  return [empty_exports, void 0];
}
function bar() {
  return [void 0];
}

export {
  foo,
  bar
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-QVTGQSXT.js
+++ rolldown	
@@ -1,8 +0,0 @@
-var empty_exports = {};
-function foo() {
-    return [empty_exports, void 0];
-}
-function bar() {
-    return [void 0];
-}
-export {foo, bar};

```