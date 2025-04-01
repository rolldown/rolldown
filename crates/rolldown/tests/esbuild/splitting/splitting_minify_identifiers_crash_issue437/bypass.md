# Reason
1. different chunk naming style
# Diff
## /out/a.js
### esbuild
```js
import {
  a as o
} from "./chunk-7N7J6VKT.js";

// a.js
console.log(o);
```
### rolldown
```js
import { foo } from "./shared.js";

//#region a.js
console.log(foo);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	a.js
@@ -1,2 +1,2 @@
-import {a as o} from "./chunk-7N7J6VKT.js";
-console.log(o);
+import {foo} from "./shared.js";
+console.log(foo);

```
## /out/b.js
### esbuild
```js
import {
  a as o
} from "./chunk-7N7J6VKT.js";

// b.js
console.log(o);
```
### rolldown
```js
import { foo } from "./shared.js";

//#region b.js
console.log(foo);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	b.js
@@ -1,2 +1,2 @@
-import {a as o} from "./chunk-7N7J6VKT.js";
-console.log(o);
+import {foo} from "./shared.js";
+console.log(foo);

```
## /out/c.js
### esbuild
```js
import "./chunk-7N7J6VKT.js";
```
### rolldown
```js
import "./shared.js";

```
### diff
```diff
===================================================================
--- esbuild	/out/c.js
+++ rolldown	c.js
@@ -1,1 +1,1 @@
-import "./chunk-7N7J6VKT.js";
+import "./shared.js";

```
## /out/chunk-7N7J6VKT.js
### esbuild
```js
// shared.js
function f(o) {
}

export {
  f as a
};
```
### rolldown
```js

//#region shared.js
function foo(bar) {}
//#endregion

export { foo };
```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-7N7J6VKT.js
+++ rolldown	shared.js
@@ -1,2 +1,2 @@
-function f(o) {}
-export {f as a};
+function foo(bar) {}
+export {foo};

```